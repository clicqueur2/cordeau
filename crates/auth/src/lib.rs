//! Mots de passe compatibles avec `llx_user.pass` de Dolibarr.
//!
//! Dolibarr ecrit `password_hash($password, PASSWORD_DEFAULT)` en PHP, soit du bcrypt
//! prefixe `$2y$`. On verifie et on produit le meme format sans PHP.
//!
//! Colonnes lues : `pass`, `pass_encoding`, `pass_crypted`.
//! - `pass_encoding` vaut `bcrypt` (ou est vide) pour les comptes recents ;
//! - `pass_crypted` porte les empreintes historiques (md5, sha1).

use md5::Digest as _;
use sha1::Sha1;

pub const BCRYPT_COST: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Bcrypt,
    Md5,
    Sha1,
    Unknown,
}

impl Encoding {
    pub fn from_column(value: Option<&str>, stored: &str) -> Self {
        if let Some(v) = value {
            let v = v.trim().to_ascii_lowercase();
            match v.as_str() {
                "bcrypt" | "" => {
                    if stored.starts_with("$2") {
                        return Encoding::Bcrypt;
                    }
                }
                "md5" => return Encoding::Md5,
                "sha1" => return Encoding::Sha1,
                _ => {}
            }
        }
        detect(stored)
    }

    pub fn as_column(&self) -> &'static str {
        match self {
            Encoding::Bcrypt => "bcrypt",
            Encoding::Md5 => "md5",
            Encoding::Sha1 => "sha1",
            Encoding::Unknown => "",
        }
    }
}

fn detect(stored: &str) -> Encoding {
    let h = stored.trim();
    if h.starts_with("$2a$")
        || h.starts_with("$2b$")
        || h.starts_with("$2x$")
        || h.starts_with("$2y$")
    {
        Encoding::Bcrypt
    } else if h.len() == 32 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        Encoding::Md5
    } else if h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        Encoding::Sha1
    } else {
        Encoding::Unknown
    }
}

/// Verifie un mot de passe contre la valeur stockee dans `llx_user.pass`
/// (ou `pass_crypted` pour les comptes historiques).
pub fn verify(password: &str, stored: &str, pass_encoding: Option<&str>) -> bool {
    let stored = stored.trim();
    if stored.is_empty() || password.is_empty() {
        return false;
    }
    match Encoding::from_column(pass_encoding, stored) {
        Encoding::Bcrypt => {
            let normalized = normalize_bcrypt(stored);
            bcrypt::verify(password, &normalized).unwrap_or(false)
        }
        Encoding::Md5 => hex_eq(&md5_hex(password.as_bytes()), stored),
        Encoding::Sha1 => hex_eq(&sha1_hex(password.as_bytes()), stored),
        Encoding::Unknown => false,
    }
}

/// Produit une empreinte au format attendu par Dolibarr (`$2y$`).
pub fn hash(password: &str) -> String {
    let h = bcrypt::hash_with_result(password, BCRYPT_COST).expect("bcrypt");
    normalize_bcrypt(&h.format_for_version(bcrypt::Version::TwoY))
}

/// `true` si l'empreinte stockee n'est plus au format courant et doit etre reecrite.
pub fn needs_rehash(stored: &str, pass_encoding: Option<&str>) -> bool {
    match Encoding::from_column(pass_encoding, stored) {
        Encoding::Bcrypt => parse_bcrypt_cost(stored)
            .map(|c| c < BCRYPT_COST)
            .unwrap_or(true),
        Encoding::Md5 | Encoding::Sha1 | Encoding::Unknown => true,
    }
}

fn parse_bcrypt_cost(h: &str) -> Option<u32> {
    let h = normalize_bcrypt(h);
    h.split('$').nth(2)?.parse().ok()
}

fn normalize_bcrypt(h: &str) -> String {
    if let Some(rest) = h.strip_prefix("$2y$") {
        format!("$2y${rest}")
    } else if let Some(rest) = h.strip_prefix("$2a$") {
        format!("$2y${rest}")
    } else if let Some(rest) = h.strip_prefix("$2b$") {
        format!("$2y${rest}")
    } else {
        h.to_string()
    }
}

fn hex_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.trim().as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i].to_ascii_lowercase() ^ b[i].to_ascii_lowercase();
    }
    diff == 0
}

fn md5_hex(input: &[u8]) -> String {
    let d = md5::Md5::digest(input);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha1_hex(input: &[u8]) -> String {
    let d = Sha1::digest(input);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Empreintes produites par PHP 8.3 `password_hash()` et `md5()`.
    const PHP_BCRYPT_12: &str = "$2y$12$.6P6wA1OuEkN3M4RqkjAMevffmmjd7gP2S77Oc3W5LyBRwJHmgQ8u";
    const PHP_BCRYPT_10: &str = "$2y$10$kRTmQxNPKsX/b6txC7dzFeXgG1h.LmapS9TX787mq9jUnjqjtQxTG";
    const PHP_MD5: &str = "e4d7da129030310de28bee199cf0cf0e";

    #[test]
    fn verifie_une_empreinte_bcrypt_produite_par_php() {
        assert!(verify("MotDePasse!2026", PHP_BCRYPT_12, Some("bcrypt")));
        assert!(!verify("MotDePasse!2027", PHP_BCRYPT_12, Some("bcrypt")));
        assert!(verify("vide", PHP_BCRYPT_10, None));
    }

    #[test]
    fn verifie_une_empreinte_md5_historique() {
        assert!(verify("ancienmotdepasse", PHP_MD5, Some("md5")));
        assert!(verify("ancienmotdepasse", PHP_MD5, None));
        assert!(!verify("autre", PHP_MD5, Some("md5")));
    }

    #[test]
    fn produit_une_empreinte_verifiable_et_au_bon_prefixe() {
        let h = hash("MotDePasse!2026");
        assert!(h.starts_with("$2y$12$"), "prefixe Dolibarr attendu: {h}");
        assert_eq!(h.len(), 60);
        assert!(verify("MotDePasse!2026", &h, Some("bcrypt")));
        assert!(!verify("MotDePasse!2025", &h, Some("bcrypt")));
    }

    #[test]
    fn deux_empreintes_du_meme_mot_de_passe_different() {
        let a = hash("secret");
        let b = hash("secret");
        assert_ne!(a, b, "le sel doit etre aleatoire");
        assert!(verify("secret", &a, None));
        assert!(verify("secret", &b, None));
    }

    #[test]
    fn refuse_les_entrees_vides_ou_inconnues() {
        assert!(!verify("", PHP_BCRYPT_12, Some("bcrypt")));
        assert!(!verify("x", "", Some("bcrypt")));
        assert!(!verify("x", "pas-un-hash", None));
        assert_eq!(
            Encoding::from_column(None, "pas-un-hash"),
            Encoding::Unknown
        );
    }

    #[test]
    fn detection_du_codage_par_la_valeur_stockee() {
        assert_eq!(Encoding::from_column(None, PHP_BCRYPT_12), Encoding::Bcrypt);
        assert_eq!(Encoding::from_column(None, PHP_MD5), Encoding::Md5);
        assert_eq!(
            Encoding::from_column(None, "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            Encoding::Sha1
        );
        assert_eq!(Encoding::Bcrypt.as_column(), "bcrypt");
        assert_eq!(Encoding::Md5.as_column(), "md5");
    }

    #[test]
    fn rehash_necessaire_pour_les_anciens_formats() {
        assert!(needs_rehash(PHP_MD5, Some("md5")));
        assert!(!needs_rehash(&hash("x"), Some("bcrypt")));
    }
}
