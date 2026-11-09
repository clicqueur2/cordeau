//! Journal en ajout seul de la caisse, hors base.
//!
//! Le scellé de `caisse_scelle` détecte une falsification **dans la base**. Ce
//! journal-ci va plus loin : il vit dans un fichier, on n'y écrit qu'en
//! ajoutant, et chaque ligne porte une empreinte qui reprend la précédente.
//!
//! # Pourquoi hors base
//!
//! Le schéma Dolibarr ne doit pas bouger — c'est la règle du projet depuis le
//! premier jour. Une table « journal » serait une table de plus. Un fichier
//! append-only ne touche à rien, se sauvegarde avec les documents, et se vérifie
//! sans MySQL.
//!
//! # Signature
//!
//! Si la variable d'environnement `DOLIBARR_SCELLE_CLE` est définie, chaque
//! ligne est signée en **HMAC-SHA256** avec cette clé. La signature prouve que
//! la ligne a été écrite par quelqu'un qui possédait la clé.
//!
//! C'est une signature **symétrique** : celui qui détient la clé peut forger une
//! ligne valide. Ce n'est donc pas une preuve contre l'administrateur, c'est une
//! preuve contre quelqu'un qui aurait un accès à la base ou au fichier sans la
//! clé. Pour aller au-delà, il faudrait une clé asymétrique et un dépositaire
//! externe — décision qui ne relève pas du code.
//!
//! # Format
//!
//! Une ligne par événement, champs séparés par `|` :
//!
//! ```text
//! <rang>|<horodatage>|<type>|<charge>|<empreinte>|<signature>
//! ```
//!
//! `empreinte` = SHA-256 de (empreinte précédente + rang + horodatage + type +
//! charge). `signature` = HMAC-SHA256 de l'empreinte, ou `-` sans clé.

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Nom du fichier de journal.
pub const FICHIER: &str = "caisse-journal.txt";

/// Clé de signature, lue dans l'environnement.
pub fn cle_depuis_environnement() -> Option<String> {
    std::env::var("DOLIBARR_SCELLE_CLE")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// Empreinte d'une ligne, chaînée à la précédente.
fn empreinte(precedente: &str, rang: u64, horodatage: &str, sorte: &str, charge: &str) -> String {
    let mut h = Sha256::new();
    h.update(precedente.as_bytes());
    h.update(b"|");
    h.update(rang.to_string().as_bytes());
    h.update(b"|");
    h.update(horodatage.as_bytes());
    h.update(b"|");
    h.update(sorte.as_bytes());
    h.update(b"|");
    h.update(charge.as_bytes());
    format!("{:x}", h.finalize())
}

/// HMAC-SHA256 artisanal : SHA-256 de (clé ^ opad || SHA-256(clé ^ ipad || msg)).
///
/// Écrit à la main plutôt qu'avec une dépendance de plus : le schéma HMAC est
/// stable depuis la RFC 2104 et tient en dix lignes.
fn hmac(cle: &str, message: &str) -> String {
    const BLOC: usize = 64;
    let mut k = [0u8; BLOC];
    let brut = cle.as_bytes();
    for (i, octet) in brut.iter().take(BLOC).enumerate() {
        k[i] = *octet;
    }
    if brut.len() > BLOC {
        let mut h = Sha256::new();
        h.update(brut);
        let resume = h.finalize();
        k = [0u8; BLOC];
        k[..32].copy_from_slice(&resume);
    }
    let mut ipad = [0u8; BLOC];
    let mut opad = [0u8; BLOC];
    for i in 0..BLOC {
        ipad[i] = k[i] ^ 0x36;
        opad[i] = k[i] ^ 0x5c;
    }
    let mut interne = Sha256::new();
    interne.update(ipad);
    interne.update(message.as_bytes());
    let interne = interne.finalize();

    let mut externe = Sha256::new();
    externe.update(opad);
    externe.update(interne);
    format!("{:x}", externe.finalize())
}

fn horodatage() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{s}")
}

/// Etat du journal apres verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rapport {
    pub lignes: u64,
    /// Empreinte de la derniere ligne, ou `GENESE` si le journal est vide.
    pub empreinte: String,
    /// Nombre de lignes signees.
    pub signees: u64,
    /// Rang de la premiere ligne fautive, s'il y en a une.
    pub premiere_erreur: Option<u64>,
    /// Pourquoi cette ligne est fautive.
    pub motif: Option<String>,
}

impl Rapport {
    pub fn intact(&self) -> bool {
        self.premiere_erreur.is_none()
    }
}

fn chemin(dossier: &Path) -> PathBuf {
    dossier.join(FICHIER)
}

fn lire(dossier: &Path) -> Result<String, Box<dyn Error>> {
    let p = chemin(dossier);
    if !p.is_file() {
        return Ok(String::new());
    }
    let mut f = File::open(&p)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    Ok(s)
}

/// Ajoute une ligne au journal. Rend (rang, empreinte).
///
/// Le rang est le nombre de lignes deja presentes + 1 : deux ecritures
/// simultanees ne peuvent pas produire le meme rang sans se voir.
pub fn ajouter(dossier: &Path, sorte: &str, charge: &str) -> Result<(u64, String), Box<dyn Error>> {
    std::fs::create_dir_all(dossier)?;
    let existant = lire(dossier)?;
    let (rang, precedente) = match existant.lines().last() {
        None => (1u64, "GENESE".to_string()),
        Some(l) => {
            let champs: Vec<&str> = l.split('|').collect();
            if champs.len() < 5 {
                return Err(format!("journal corrompu, derniere ligne illisible : {l}").into());
            }
            let rang: u64 = champs[0].parse()?;
            (rang + 1, champs[4].to_string())
        }
    };

    // La charge ne doit pas contenir le separateur : « POS-1|10.00 » casserait
    // la relecture. On remplace le trait vertical par un point-virgule.
    let charge = &charge.replace('|', ";");
    let horodatage = horodatage();
    let emp = empreinte(&precedente, rang, &horodatage, sorte, charge);
    let sig = match cle_depuis_environnement() {
        Some(cle) => hmac(&cle, &emp),
        None => "-".to_string(),
    };

    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(chemin(dossier))?;
    writeln!(f, "{rang}|{horodatage}|{sorte}|{charge}|{emp}|{sig}")?;
    f.flush()?;
    Ok((rang, emp))
}

/// Re-verifie tout le journal : chaine des empreintes et signatures.
///
/// Une ligne modifiee, inseree ou retiree casse la chaine. Une ligne dont la
/// signature ne correspond pas est signalee : la cle a change, ou la ligne a
/// ete fabriquee sans elle.
pub fn verifier(dossier: &Path) -> Result<Rapport, Box<dyn Error>> {
    let contenu = lire(dossier)?;
    let cle = cle_depuis_environnement();

    let mut precedente = "GENESE".to_string();
    let mut attendu = 1u64;
    let mut lignes = 0u64;
    let mut signees = 0u64;

    for l in contenu.lines() {
        if l.trim().is_empty() {
            continue;
        }
        let champs: Vec<&str> = l.split('|').collect();
        if champs.len() != 6 {
            return Ok(Rapport {
                lignes,
                empreinte: precedente,
                signees,
                premiere_erreur: Some(attendu),
                motif: Some(format!("ligne malformee : {l}")),
            });
        }
        let rang: u64 = champs[0].parse().unwrap_or(0);
        if rang != attendu {
            return Ok(Rapport {
                lignes,
                empreinte: precedente,
                signees,
                premiere_erreur: Some(attendu),
                motif: Some(format!("rang attendu {attendu}, trouve {rang}")),
            });
        }
        let calculee = empreinte(&precedente, rang, champs[1], champs[2], champs[3]);
        if calculee != champs[4] {
            return Ok(Rapport {
                lignes,
                empreinte: precedente,
                signees,
                premiere_erreur: Some(rang),
                motif: Some("empreinte ne correspond pas au contenu".to_string()),
            });
        }
        if let Some(cle) = &cle {
            if champs[5] == "-" {
                return Ok(Rapport {
                    lignes,
                    empreinte: precedente,
                    signees,
                    premiere_erreur: Some(rang),
                    motif: Some("ligne non signee alors que la cle est presente".to_string()),
                });
            }
            if hmac(cle, champs[4]) != champs[5] {
                return Ok(Rapport {
                    lignes,
                    empreinte: precedente,
                    signees,
                    premiere_erreur: Some(rang),
                    motif: Some("signature invalide".to_string()),
                });
            }
            signees += 1;
        }

        precedente = champs[4].to_string();
        lignes += 1;
        attendu += 1;
    }

    Ok(Rapport {
        lignes,
        empreinte: precedente,
        signees,
        premiere_erreur: None,
        motif: None,
    })
}

/// Le dossier du journal : `DOLIBARR_JOURNAL_CAISSE`, sinon
/// `<documents>/journal-caisse`.
///
/// Il vit a cote des documents et non en base : une chaine de hachages qu'on
/// peut reecrire par SQL ne prouve rien.
pub fn dossier() -> std::path::PathBuf {
    if let Ok(v) = std::env::var("DOLIBARR_JOURNAL_CAISSE") {
        if !v.trim().is_empty() {
            return std::path::PathBuf::from(v);
        }
    }
    crate::ged::racine().join("journal-caisse")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier(prefixe: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("{prefixe}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn le_journal_se_lit_et_se_verifie() {
        let d = dossier("journal-ok");
        let (r1, _) = ajouter(&d, "VENTE", "POS-1|10.00").unwrap();
        let (r2, _) = ajouter(&d, "VENTE", "POS-2|20.00").unwrap();
        assert_eq!((r1, r2), (1, 2));

        let rap = verifier(&d).unwrap();
        assert!(rap.intact(), "motif : {:?}", rap.motif);
        assert_eq!(rap.lignes, 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn une_ligne_modifiee_casse_la_chaine() {
        let d = dossier("journal-modif");
        ajouter(&d, "VENTE", "POS-1|10.00").unwrap();
        ajouter(&d, "VENTE", "POS-2|20.00").unwrap();

        let p = chemin(&d);
        let contenu = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, contenu.replace("POS-1;10.00", "POS-1;10.01")).unwrap();

        let rap = verifier(&d).unwrap();
        assert!(!rap.intact(), "la modification doit etre vue");
        assert_eq!(rap.premiere_erreur, Some(1));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn une_ligne_retiree_casse_la_numerotation() {
        let d = dossier("journal-suppr");
        ajouter(&d, "VENTE", "POS-1|10.00").unwrap();
        ajouter(&d, "VENTE", "POS-2|20.00").unwrap();
        ajouter(&d, "VENTE", "POS-3|30.00").unwrap();

        let p = chemin(&d);
        let lignes: Vec<String> = std::fs::read_to_string(&p)
            .unwrap()
            .lines()
            .map(String::from)
            .collect();
        std::fs::write(&p, format!("{}\n{}\n", lignes[0], lignes[2])).unwrap();

        let rap = verifier(&d).unwrap();
        assert!(!rap.intact());
        assert_eq!(rap.premiere_erreur, Some(2), "le rang 2 manque");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_signature_suit_la_cle() {
        let d = dossier("journal-hmac");
        std::env::set_var("DOLIBARR_SCELLE_CLE", "cle-de-test");
        ajouter(&d, "VENTE", "POS-1|10.00").unwrap();
        let rap = verifier(&d).unwrap();
        assert!(rap.intact());
        assert_eq!(rap.signees, 1, "la ligne doit etre signee");

        // Une autre cle : la signature ne correspond plus.
        std::env::set_var("DOLIBARR_SCELLE_CLE", "autre-cle");
        let rap2 = verifier(&d).unwrap();
        assert!(!rap2.intact());
        assert_eq!(rap2.motif.as_deref(), Some("signature invalide"));

        std::env::remove_var("DOLIBARR_SCELLE_CLE");
        let _ = std::fs::remove_dir_all(&d);
    }
}
