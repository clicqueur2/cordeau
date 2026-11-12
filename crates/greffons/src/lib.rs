//! Greffons : brancher du code externe sans recompiler le noyau.
//!
//! Dolibarr appelle des fonctions PHP par hooks. Rust n'a pas d'ABI stable, donc
//! un greffon ne peut pas etre un crate charge a chaud. Il peut etre une
//! **bibliotheque dynamique en C ABI** : c'est la seule frontiere stable que
//! Rust garantit.
//!
//! # Ce contrat n'est pas « celui de tous les langages »
//!
//! Il transporte une **allocation** : `dolibarr_greffon` rend un `char *` que le
//! noyau libere lui-meme. Cela ne fonctionne que si le greffon et le noyau
//! partagent le **meme allocateur**. Sous Windows, une `cdylib` Rust liee a la
//! CRT statique a **son propre tas** : un greffon ecrit en C, avec le `malloc`
//! de son processus, provoque un free inter-tas, fatal et silencieux.
//!
//! La correction propre — un tampon de sortie fourni par le noyau — supprimerait
//! le troisieme symbole. Elle n'est pas faite : le contrat tel qu'il est ecrit
//! vaut **pour un greffon ecrit en Rust**, pas pour un `.c` arbitraire.
//!
//! # Le contrat
//!
//! Un greffon est un `.dll` (Windows) ou un `.so` (Linux) qui exporte
//! exactement trois symboles :
//!
//! ```c
//! unsigned int dolibarr_api_version(void);          // doit valoir 1
//! char *dolibarr_greffon(const char *evenement);    // JSON entrant, JSON sortant
//! void  dolibarr_liberer(char *chaine);             // libere ce que le greffon a rendu
//! ```
//!
//! Le noyau refuse un greffon dont la version ne correspond pas : c'est ce qui
//! evite qu'une mise a jour du noyau casse silencieusement une extension.
//!
//! # Ce qu'un greffon ne peut pas faire
//!
//! Il recoit un evenement et rend une decision. Il ne dessine pas d'ecran, ne
//! modifie pas la base directement : pour cela il passe par l'API REST, comme
//! n'importe quel client.

use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};

/// La version d'API que ce noyau sait parler.
pub const API: u32 = 1;

/// Ce qui a echoue au chargement ou a l'appel.
#[derive(Debug)]
pub enum Erreur {
    /// Le fichier n'a pas pu etre ouvert (mauvaise architecture, dependance
    /// manquante).
    Chargement(String),
    /// Un des trois symboles manque.
    Symbole(String),
    /// Le greffon annonce une autre version d'API.
    Version(u32),
    /// Le greffon a rendu du vide ou du non-UTF8.
    Reponse(String),
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::Chargement(e) => write!(f, "chargement impossible : {e}"),
            Erreur::Symbole(s) => write!(f, "symbole manquant : {s}"),
            Erreur::Version(v) => {
                write!(f, "greffon en API {v}, le noyau parle {API}")
            }
            Erreur::Reponse(e) => write!(f, "reponse illisible : {e}"),
        }
    }
}

impl std::error::Error for Erreur {}

type Version = unsafe extern "C" fn() -> u32;
type Appel = unsafe extern "C" fn(*const std::os::raw::c_char) -> *mut std::os::raw::c_char;
type Liberer = unsafe extern "C" fn(*mut std::os::raw::c_char);

/// Un greffon charge, garde vivant tant qu'on peut l'appeler.
pub struct Greffon {
    pub nom: String,
    pub chemin: PathBuf,
    bibliotheque: Library,
}

impl Greffon {
    /// Charge un greffon et verifie sa version d'API.
    pub fn charger(chemin: &Path) -> Result<Self, Erreur> {
        // La bibliotheque vit aussi longtemps que le greffon : les symboles
        // qu'on en tire ne doivent pas lui survivre.
        let bibliotheque =
            unsafe { Library::new(chemin) }.map_err(|e| Erreur::Chargement(e.to_string()))?;

        let version: Symbol<Version> = unsafe { bibliotheque.get(b"dolibarr_api_version\0") }
            .map_err(|_| Erreur::Symbole("dolibarr_api_version".into()))?;
        let v = unsafe { version() };
        if v != API {
            return Err(Erreur::Version(v));
        }
        // Les deux autres symboles sont exiges des le chargement : mieux vaut
        // refuser un greffon incomplet que le decouvrir a la premiere facture.
        unsafe { bibliotheque.get::<Appel>(b"dolibarr_greffon\0") }
            .map_err(|_| Erreur::Symbole("dolibarr_greffon".into()))?;
        unsafe { bibliotheque.get::<Liberer>(b"dolibarr_liberer\0") }
            .map_err(|_| Erreur::Symbole("dolibarr_liberer".into()))?;

        let nom = chemin
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "greffon".into());

        Ok(Self {
            nom,
            chemin: chemin.to_path_buf(),
            bibliotheque,
        })
    }

    /// Envoie un evenement au greffon et rend sa reponse telle quelle.
    ///
    /// L'evenement est du JSON : le greffon le lit comme il veut, sans
    /// connaitre les types Rust du noyau.
    pub fn appeler(&self, evenement: &str) -> Result<String, Erreur> {
        let entree = CString::new(evenement)
            .map_err(|_| Erreur::Reponse("evenement avec un octet nul".into()))?;

        let appel: Symbol<Appel> = unsafe { self.bibliotheque.get(b"dolibarr_greffon\0") }
            .map_err(|_| Erreur::Symbole("dolibarr_greffon".into()))?;
        let liberer: Symbol<Liberer> = unsafe { self.bibliotheque.get(b"dolibarr_liberer\0") }
            .map_err(|_| Erreur::Symbole("dolibarr_liberer".into()))?;

        let sortie = unsafe { appel(entree.as_ptr()) };
        if sortie.is_null() {
            return Err(Erreur::Reponse("le greffon n'a rien rendu".into()));
        }
        let texte = unsafe { CStr::from_ptr(sortie) }
            .to_str()
            .map(|s| s.to_string())
            .map_err(|e| Erreur::Reponse(e.to_string()));
        // La chaine appartient au greffon : c'est lui qui la libere.
        unsafe { liberer(sortie) };
        texte
    }
}

/// Charge tous les greffons d'un dossier (`.dll`, `.so`, `.dylib`).
///
/// Un fichier casse est signale et ignore : un greffon ne doit pas empecher le
/// serveur de demarrer.
pub fn charger_dossier(dossier: &Path) -> Result<Vec<Greffon>, Erreur> {
    if !dossier.is_dir() {
        return Ok(Vec::new());
    }
    let mut greffons = Vec::new();
    let entrees = std::fs::read_dir(dossier).map_err(|e| Erreur::Chargement(e.to_string()))?;
    let mut chemins: Vec<PathBuf> = entrees
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("dll") | Some("so") | Some("dylib")
            )
        })
        .collect();
    chemins.sort();

    for chemin in chemins {
        match Greffon::charger(&chemin) {
            Ok(g) => greffons.push(g),
            Err(e) => eprintln!("greffon ignore ({}): {e}", chemin.display()),
        }
    }
    Ok(greffons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_dossier_absent_ne_charge_rien() {
        let rien = charger_dossier(Path::new("chemin/qui/n/existe/pas")).unwrap();
        assert!(rien.is_empty());
    }

    #[test]
    fn un_fichier_qui_n_est_pas_une_bibliotheque_est_refuse() {
        let dossier = std::env::temp_dir().join("greffons_test_faux");
        std::fs::create_dir_all(&dossier).unwrap();
        let faux = dossier.join("faux.dll");
        std::fs::write(&faux, b"ceci n'est pas une bibliotheque").unwrap();
        assert!(matches!(
            Greffon::charger(&faux),
            Err(Erreur::Chargement(_))
        ));
        let _ = std::fs::remove_file(&faux);
    }

    /// Le vrai test : on charge le greffon d'exemple s'il a ete compile.
    ///
    /// Il ne s'agit pas d'un test unitaire classique : il prouve la chaine
    /// complete (chargement, version d'API, appel C ABI, JSON). Si le greffon
    /// n'est pas compile sur cette machine, le test est ignore plutot que
    /// rouge — comme les tests de base de donnees quand MariaDB est absente.
    #[test]
    fn le_greffon_d_exemple_repond() {
        let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        let Some(racine) = racine else {
            return;
        };
        let cible = std::env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| racine.join("target"));
        let nom = if cfg!(windows) {
            "greffon_exemple.dll"
        } else {
            "libgreffon_exemple.so"
        };
        let chemin = cible.join("debug").join(nom);
        if !chemin.is_file() {
            eprintln!(
                "greffon d'exemple non compile : test ignore ({})",
                chemin.display()
            );
            return;
        }

        let greffon = Greffon::charger(&chemin).expect("chargement du greffon d'exemple");
        let reponse = greffon
            .appeler(r#"{"evenement":"facture.validee","reference":"F-1"}"#)
            .expect("appel du greffon");
        assert!(
            reponse.contains("\"ok\":true"),
            "reponse inattendue : {reponse}"
        );
        assert!(
            reponse.contains("F-1"),
            "la reference doit revenir : {reponse}"
        );
    }
}
