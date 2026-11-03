//! Chargement des vrais fichiers de traduction de Dolibarr.
//!
//! Dolibarr range ses traductions dans `htdocs/langs/<langue>/*.lang`, un
//! dossier par langue (`fr_FR`, `en_US`, `es_ES`...) et un fichier par module
//! (`main.lang`, `bills.lang`, `propal.lang`...). Le format des lignes est
//! celui que comprend deja [`Catalogue::depuis_texte`] : `Cle=valeur`,
//! commentaires `#`, sequences `\n` converties.
//!
//! Ce module apporte ce que le catalogue ne peut pas deviner : le disque.
//!
//! ```no_run
//! use i18n::Catalogue;
//! use i18n::dolibarr::charger_langue;
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let racine = Path::new("_vendor/dolibarr-src/htdocs/langs");
//! let mut catalogue = Catalogue::nouveau("fr");
//! let nb = charger_langue(&mut catalogue, racine, "fr_FR")?;
//! println!("{nb} cles chargees");
//! # let _ = catalogue.t("fr", "Invoices");
//! # Ok(())
//! # }
//! ```
//!
//! # Ce que le dossier de langue contient vraiment
//!
//! Verifie sur Dolibarr 24.0.1 (`_vendor/dolibarr-src/htdocs/langs/fr_FR`,
//! 71 fichiers) :
//!
//! - les fichiers sont en **UTF-8 sans BOM** et en fins de ligne Unix, mais
//!   rien ne le garantit dans les paquets de langue plus anciens : le BOM est
//!   retire et les `\r\n` sont absorbees ;
//! - les cles sont parfois **entourees d'espaces** avant le `=` et les valeurs
//!   apres (`BookkeepingNumRefModelDesc = Renvoie le numero...`) : le catalogue
//!   rogne les deux, donc ces lignes se chargent normalement ;
//! - la meme cle peut apparaitre dans **deux fichiers** (66 collisions en
//!   `fr_FR`) ; le dernier fichier lu l'emporte, et c'est pourquoi l'ordre
//!   alphabetique des fichiers est impose ici ;
//! - tous les fichiers du dossier ne sont pas des traductions : seuls les
//!   `*.lang` sont lus, un sous-dossier nomme `quelque-chose.lang` est ignore.
//!
//! # L'encodage
//!
//! Certains paquets de langue Dolibarr sont encore en ISO-8859-1. Un fichier
//! qui n'est pas de l'UTF-8 valide est donc relu en latin-1, ou chaque octet
//! donne exactement un caractere : rien n'est perdu, rien n'est remplace par
//! un losange. Les 71 fichiers francais de 24.0.1 sont, eux, en UTF-8.

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Catalogue;

/// Extension des fichiers de traduction.
const EXTENSION: &str = "lang";

/// Longueur maximale acceptee pour un nom de dossier de langue, garde-fou
/// contre un code absurde venu d'un en-tete HTTP.
const LONGUEUR_CODE_MAX: usize = 24;

/// Ce qui empeche de lire un dossier de langues.
#[derive(Debug)]
pub enum ErreurFichiers {
    /// La racine `htdocs/langs` elle-meme est introuvable ou illisible.
    RacineIllisible { racine: PathBuf, source: io::Error },
    /// Le dossier de la langue existe mais son contenu ne se lit pas.
    DossierIllisible { dossier: PathBuf, source: io::Error },
    /// Le code de langue ne peut pas etre un nom de dossier : vide, fait de
    /// separateurs de chemin, de points.
    LangueInvalide { langue: String },
    /// Aucun dossier ne porte ce nom de langue : la demander est une erreur,
    /// pas un chargement de zero cle.
    LangueInconnue { langue: String, dossier: PathBuf },
    /// Un fichier `.lang` n'a pas pu etre lu.
    FichierIllisible { fichier: PathBuf, source: io::Error },
    /// Toutes les langues demandees ont echoue ; le detail de chacune est
    /// conserve pour que l'appelant sache laquelle corriger.
    AucuneLangueChargee { raisons: Vec<String> },
}

impl std::fmt::Display for ErreurFichiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErreurFichiers::RacineIllisible { racine, .. } => {
                write!(f, "dossier des langues illisible : {}", racine.display())
            }
            ErreurFichiers::DossierIllisible { dossier, .. } => {
                write!(f, "dossier de langue illisible : {}", dossier.display())
            }
            ErreurFichiers::LangueInvalide { langue } => {
                write!(f, "code de langue invalide : {langue:?}")
            }
            ErreurFichiers::LangueInconnue { langue, dossier } => write!(
                f,
                "langue {langue} absente : aucun dossier {}",
                dossier.display()
            ),
            ErreurFichiers::FichierIllisible { fichier, .. } => {
                write!(f, "fichier de langue illisible : {}", fichier.display())
            }
            ErreurFichiers::AucuneLangueChargee { raisons } => {
                if raisons.is_empty() {
                    write!(f, "aucune langue demandee au chargement")
                } else {
                    write!(f, "aucune langue chargee ({})", raisons.join(" ; "))
                }
            }
        }
    }
}

impl Error for ErreurFichiers {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ErreurFichiers::RacineIllisible { source, .. }
            | ErreurFichiers::DossierIllisible { source, .. }
            | ErreurFichiers::FichierIllisible { source, .. } => Some(source),
            ErreurFichiers::LangueInvalide { .. }
            | ErreurFichiers::LangueInconnue { .. }
            | ErreurFichiers::AucuneLangueChargee { .. } => None,
        }
    }
}

/// Verifie qu'un code peut servir de nom de dossier, et rien de plus.
///
/// On refuse ce qui remonterait l'arborescence (`..`, `/`, `\`) : le code de
/// langue vient souvent d'un en-tete HTTP, il ne doit jamais pouvoir designer
/// autre chose qu'un dossier de `htdocs/langs`.
fn verifier_code(langue: &str) -> Result<(), ErreurFichiers> {
    let propre = langue.trim();
    let acceptable = !propre.is_empty()
        && propre.len() <= LONGUEUR_CODE_MAX
        && propre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if acceptable {
        Ok(())
    } else {
        Err(ErreurFichiers::LangueInvalide {
            langue: langue.to_string(),
        })
    }
}

/// Vrai si le chemin finit par `.lang`, comme `main.lang`.
///
/// La comparaison est sensible a la casse, comme le glob `*.lang` de Dolibarr.
fn est_fichier_lang(chemin: &Path) -> bool {
    chemin.extension() == Some(OsStr::new(EXTENSION))
}

/// Les fichiers `.lang` d'un dossier, tries par nom.
///
/// Le tri rend le chargement reproductible : une cle presente dans deux
/// fichiers est toujours ecrasee par le meme, quel que soit l'ordre rendu par
/// le systeme de fichiers.
fn fichiers_lang(dossier: &Path) -> Result<Vec<PathBuf>, ErreurFichiers> {
    let entrees = fs::read_dir(dossier).map_err(|source| ErreurFichiers::DossierIllisible {
        dossier: dossier.to_path_buf(),
        source,
    })?;
    let mut fichiers = Vec::new();
    for entree in entrees {
        let entree = entree.map_err(|source| ErreurFichiers::DossierIllisible {
            dossier: dossier.to_path_buf(),
            source,
        })?;
        let type_entree =
            entree
                .file_type()
                .map_err(|source| ErreurFichiers::DossierIllisible {
                    dossier: dossier.to_path_buf(),
                    source,
                })?;
        // Un sous-dossier peut porter une extension trompeuse (`vieux.lang`) :
        // seul un vrai fichier est une source de traductions.
        if !type_entree.is_file() {
            continue;
        }
        let chemin = entree.path();
        if est_fichier_lang(&chemin) {
            fichiers.push(chemin);
        }
    }
    fichiers.sort();
    Ok(fichiers)
}

/// Transforme les octets d'un fichier de langue en texte.
///
/// Le BOM UTF-8 est retire **avant** tout essai de decodage : relu en latin-1
/// il deviendrait trois caracteres parasites colles devant la premiere cle.
/// Un contenu qui n'est pas de l'UTF-8 valide est relu en latin-1, sans perte.
fn decoder(octets: &[u8]) -> String {
    let octets = if octets.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &octets[3..]
    } else {
        octets
    };
    match std::str::from_utf8(octets) {
        Ok(texte) => texte.to_string(),
        Err(_) => octets.iter().map(|octet| char::from(*octet)).collect(),
    }
}

/// Charge tous les `*.lang` d'une langue dans `cat`, et rend le nombre de
/// lignes de traduction lues.
///
/// `langue` est le **nom du dossier** de Dolibarr (`fr_FR`), pas le code
/// normalise : c'est ce qui permet de le trouver sur le disque. Le catalogue,
/// lui, enregistre sous le code principal (`fr`), donc `t("fr", ...)` et
/// `t("fr_FR", ...)` retrouvent la meme traduction.
///
/// Un dossier de langue absent est une **erreur**, pas un chargement de zero
/// cle : une interface qui afficherait des cles brutes sans que personne ne
/// sache pourquoi serait plus difficile a diagnostiquer qu'un echec net.
///
/// Le compte rendu additionne les lignes lues fichier par fichier ; comme une
/// meme cle peut figurer dans deux fichiers, il peut depasser le nombre de
/// cles distinctes du catalogue (voir [`Catalogue::nb_cles`]).
pub fn charger_langue(
    cat: &mut Catalogue,
    racine: &Path,
    langue: &str,
) -> Result<usize, Box<dyn Error>> {
    verifier_code(langue)?;
    let dossier = racine.join(langue.trim());
    if !dossier.is_dir() {
        return Err(Box::new(ErreurFichiers::LangueInconnue {
            langue: langue.to_string(),
            dossier,
        }));
    }
    let mut total = 0;
    for fichier in fichiers_lang(&dossier)? {
        let octets = fs::read(&fichier).map_err(|source| ErreurFichiers::FichierIllisible {
            fichier: fichier.clone(),
            source,
        })?;
        total += cat.depuis_texte(langue, &decoder(&octets));
    }
    Ok(total)
}

/// Charge plusieurs langues d'affilee, et rend `(langue, nombre de cles)` pour
/// celles qui ont abouti.
///
/// **Une langue en echec n'empeche pas les autres** : son erreur est ecrite sur
/// la sortie d'erreur avec son motif, la boucle continue, et les langues
/// valides restent chargees. C'est le comportement utile au demarrage d'un
/// serveur : une langue au paquet abime ne doit pas priver l'utilisateur des
/// quatre autres.
///
/// Si **aucune** langue n'a abouti, l'appel rend une erreur qui rassemble tous
/// les motifs : un `Vec` vide en `Ok` laisserait croire a un succes.
pub fn charger_langues(
    cat: &mut Catalogue,
    racine: &Path,
    langues: &[&str],
) -> Result<Vec<(String, usize)>, Box<dyn Error>> {
    let mut chargees = Vec::new();
    let mut raisons = Vec::new();
    for langue in langues.iter().copied() {
        match charger_langue(cat, racine, langue) {
            Ok(nb) => chargees.push((langue.to_string(), nb)),
            Err(erreur) => {
                eprintln!("langue {langue} non chargee : {erreur}");
                raisons.push(format!("{langue} : {erreur}"));
            }
        }
    }
    if chargees.is_empty() {
        return Err(Box::new(ErreurFichiers::AucuneLangueChargee { raisons }));
    }
    Ok(chargees)
}

/// Les langues installees sous `racine`, triees.
///
/// Ne retient que les dossiers qui portent au moins un `*.lang` : un dossier
/// vide ou un reste de sauvegarde ne sont pas des langues chargeables. Les
/// dossiers caches sont ecartes, et un nom qui ne peut pas etre un code de
/// langue (points, separateurs) aussi.
pub fn langues_disponibles(racine: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let entrees = fs::read_dir(racine).map_err(|source| ErreurFichiers::RacineIllisible {
        racine: racine.to_path_buf(),
        source,
    })?;
    let mut langues = Vec::new();
    for entree in entrees {
        let entree = entree.map_err(|source| ErreurFichiers::RacineIllisible {
            racine: racine.to_path_buf(),
            source,
        })?;
        let est_dossier = entree
            .file_type()
            .map_err(|source| ErreurFichiers::RacineIllisible {
                racine: racine.to_path_buf(),
                source,
            })?
            .is_dir();
        if !est_dossier {
            continue;
        }
        let chemin = entree.path();
        let Some(nom) = chemin.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if nom.starts_with('.') || verifier_code(nom).is_err() {
            continue;
        }
        let chargeable = fichiers_lang(&chemin).is_ok_and(|fichiers| !fichiers.is_empty());
        if chargeable {
            langues.push(nom.to_string());
        }
    }
    langues.sort();
    Ok(langues)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le dossier des langues du Dolibarr de reference, s'il a ete clone.
    fn racine_vendor() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("_vendor")
            .join("dolibarr-src")
            .join("htdocs")
            .join("langs")
    }

    /// Un dossier de travail propre et unique par test.
    fn racine_temporaire(nom: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("i18n-lang-{nom}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("creation du dossier temporaire");
        base
    }

    #[test]
    fn charge_le_vrai_francais() {
        let racine = racine_vendor();
        let dossier = racine.join("fr_FR");
        if !dossier.is_dir() {
            eprintln!(
                "test saute : {} absent (depot clone sans _vendor)",
                dossier.display()
            );
            return;
        }

        let mut cat = Catalogue::nouveau("fr");
        let nb = charger_langue(&mut cat, &racine, "fr_FR").expect("le francais doit se charger");

        eprintln!(
            "fr_FR : {nb} lignes lues, {} cles distinctes",
            cat.nb_cles("fr")
        );
        assert!(
            nb >= 1000,
            "seulement {nb} cles lues dans {}",
            dossier.display()
        );

        // Cle verifiee presente telle quelle dans `bills.lang`.
        let texte = cat.t("fr", "Invoices");
        assert!(!texte.is_empty(), "traduction vide pour Invoices");
        assert_ne!(texte, "Invoices", "la cle est rendue au lieu du texte");
        assert_eq!(texte, "Factures");
        // Le dossier s'appelle fr_FR, le catalogue repond aussi sous fr.
        assert_eq!(cat.t("fr_FR", "Invoices"), "Factures");
    }

    #[test]
    fn langues_disponibles_trouve_fr_fr() {
        let racine = racine_vendor();
        if !racine.is_dir() {
            eprintln!("test saute : {} absent", racine.display());
            return;
        }

        let langues = langues_disponibles(&racine).expect("la racine doit se lire");
        eprintln!("langues installees : {langues:?}");
        assert!(
            langues.iter().any(|langue| langue == "fr_FR"),
            "fr_FR absent de {langues:?}"
        );
        let mut triees = langues.clone();
        triees.sort();
        assert_eq!(langues, triees, "la liste doit etre triee");
    }

    #[test]
    fn langue_inexistante_rend_une_erreur() {
        let racine = racine_temporaire("inexistante");
        fs::create_dir_all(racine.join("fr_FR")).expect("dossier fr_FR");
        fs::write(racine.join("fr_FR").join("main.lang"), "Bonjour=Salut\n").expect("ecriture");

        let mut cat = Catalogue::nouveau("fr");
        let resultat = charger_langue(&mut cat, &racine, "zz_ZZ");
        let erreur = resultat.expect_err("une langue absente doit echouer");
        let message = erreur.to_string();
        assert!(message.contains("zz_ZZ"), "message peu parlant : {message}");
        // Rien n'a ete charge au passage.
        assert_eq!(cat.nb_cles("fr"), 0);

        // Un code qui sortirait de l'arborescence est refuse avant tout acces
        // au disque.
        assert!(charger_langue(&mut cat, &racine, "../..").is_err());
        assert!(charger_langue(&mut cat, &racine, "").is_err());

        let _ = fs::remove_dir_all(&racine);
    }

    #[test]
    fn une_langue_en_echec_n_empeche_pas_les_autres() {
        let racine = racine_temporaire("partiel");
        let fr = racine.join("fr_FR");
        fs::create_dir_all(&fr).expect("dossier fr_FR");

        // Fichier en UTF-8 avec BOM et fins de ligne Windows.
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(b"Bonjour=Salut\r\nDeux=Ligne 1\\nLigne 2\r\n");
        fs::write(fr.join("01-accueil.lang"), bom).expect("fichier 01");

        // Meme cle dans un fichier suivant : l'ordre alphabetique tranche.
        fs::write(fr.join("02-doublon.lang"), "Bonjour=Bonsoir\r\n").expect("fichier 02");

        // Fichier en ISO-8859-1 : le 0xE9 isole est un « e » accentue.
        fs::write(fr.join("03-latin1.lang"), b"Decaissement=5\xE9\r\n").expect("fichier 03");

        // Ni un .txt ni un dossier nomme *.lang ne doivent etre lus.
        fs::write(fr.join("notes.txt"), "Fantome=Jamais\n").expect("fichier texte");
        let piege = fr.join("04-piege.lang");
        fs::create_dir_all(&piege).expect("dossier piege");
        fs::write(piege.join("interne.lang"), "Interne=Cache\n").expect("fichier interne");

        let mut cat = Catalogue::nouveau("fr");
        let chargees = charger_langues(&mut cat, &racine, &["fr_FR", "zz_ZZ", "yy_YY"])
            .expect("le francais doit aboutir malgre les autres");

        // Seule la langue valide est rendue, avec son compte.
        assert_eq!(chargees, vec![("fr_FR".to_string(), 4)]);
        assert_eq!(cat.nb_cles("fr"), 3);
        assert_eq!(cat.t("fr", "Bonjour"), "Bonsoir");
        assert_eq!(cat.t("fr", "Deux"), "Ligne 1\nLigne 2");
        assert_eq!(cat.t("fr", "Decaissement"), "5\u{e9}");
        // Les fichiers non .lang et les dossiers n'ont rien apporte.
        assert_eq!(cat.t("fr", "Fantome"), "Fantome");
        assert_eq!(cat.t("fr", "Interne"), "Interne");

        // Toutes les langues fautives : l'erreur rassemble les motifs.
        let mut vide = Catalogue::nouveau("fr");
        let erreur = charger_langues(&mut vide, &racine, &["zz_ZZ", "yy_YY"])
            .expect_err("aucune langue valide ne doit echouer");
        let message = erreur.to_string();
        assert!(message.contains("zz_ZZ"), "motif incomplet : {message}");
        assert!(message.contains("yy_YY"), "motif incomplet : {message}");

        // Une liste vide n'est pas un succes silencieux.
        assert!(charger_langues(&mut vide, &racine, &[]).is_err());

        let _ = fs::remove_dir_all(&racine);
    }

    #[test]
    fn racine_absente_signalee() {
        let racine = racine_temporaire("absente").join("pas-la");
        assert!(langues_disponibles(&racine).is_err());

        let mut cat = Catalogue::nouveau("fr");
        assert!(charger_langue(&mut cat, &racine, "fr_FR").is_err());
    }
}
