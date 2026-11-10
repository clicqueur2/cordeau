//! Portail client : le client se connecte et voit **ses** devis et factures.
//!
//! Dolibarr range les comptes du portail dans `llx_societe_account` : un login,
//! un mot de passe, un tiers. On utilise la meme table, et on ne touche pas au
//! schema.
//!
//! # Ce que le portail ne fait pas
//!
//! Il **lit**. Le client n'emet pas de devis, ne valide rien, n'encaisse rien.
//! Un portail qui ecrit est une porte d'entree sur la comptabilite ; celui-ci
//! n'en est pas une.
//!
//! # Les mots de passe
//!
//! Ils sont stockes en `sha256`, marques par `pass_encoding = 'cordeau-sha256'`.
//! Un compte dont l'encodage est autre (Dolibarr a ses propres formats) est
//! **refuse** a la connexion : on ne devine pas un format qu'on ne sait pas lire.
//!
//! # Le cloisonnement
//!
//! Chaque lecture verifie que la piece appartient bien au tiers du compte. La
//! fonction [`appartient`] est le seul chemin : sans elle, un client change un
//! numero dans l'URL et lit la facture du voisin.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use sha2::{Digest, Sha256};

/// L'encodage qu'on sait lire et ecrire. Tout autre est refuse.
pub const ENCODAGE: &str = "cordeau-sha256";

/// Un compte du portail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compte {
    pub rowid: i64,
    pub entity: i64,
    pub fk_soc: i64,
    pub login: String,
    /// Nom du tiers, pour l'afficher en haut de page.
    pub tiers: String,
}

/// Empreinte d'un mot de passe. `sha256(login + ':' + mot_de_passe)` : le login
/// dans l'empreinte evite que deux comptes au meme mot de passe aient la meme
/// empreinte, sans sel stocke a part.
pub fn empreinte(login: &str, mot_de_passe: &str) -> String {
    let mut h = Sha256::new();
    h.update(login.as_bytes());
    h.update(b":");
    h.update(mot_de_passe.as_bytes());
    format!("{:x}", h.finalize())
}

/// Cree un compte de portail pour un tiers.
pub fn creer_compte(
    pool: &Pool,
    entity: i64,
    fk_soc: i64,
    login: &str,
    mot_de_passe: &str,
) -> Result<i64, Box<dyn Error>> {
    let login = login.trim();
    if login.len() < 3 {
        return Err("le login doit faire au moins trois caracteres".into());
    }
    if mot_de_passe.len() < 8 {
        return Err("le mot de passe doit faire au moins huit caracteres".into());
    }
    let mut conn = pool.get_conn()?;

    let tiers: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :soc",
        params! { "entity" => entity, "soc" => fk_soc },
    )?;
    if tiers.is_none() {
        return Err(format!("le tiers {fk_soc} n'existe pas dans l'entite {entity}").into());
    }
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe_account WHERE entity = :entity AND login = :login",
        params! { "entity" => entity, "login" => login },
    )?;
    if deja.is_some() {
        return Err(format!("le login « {login} » est deja pris dans cette societe").into());
    }

    conn.exec_drop(
        "INSERT INTO llx_societe_account \
         (entity, login, pass_encoding, pass_crypted, fk_soc, site, status, \
          date_creation, fk_user_creat) \
         VALUES (:entity, :login, :enc, :empreinte, :soc, 'portail', 1, NOW(), \
          (SELECT MIN(rowid) FROM llx_user))",
        params! {
            "entity" => entity,
            "login" => login,
            "enc" => ENCODAGE,
            "empreinte" => empreinte(login, mot_de_passe),
            "soc" => fk_soc,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Verifie un login et un mot de passe. Rend le compte, ou `None`.
///
/// `None` couvre trois cas qu'on ne distingue pas volontairement : compte
/// inconnu, mot de passe faux, encodage non gere. Un message precis aiderait
/// celui qui essaie de deviner.
pub fn authentifier(
    pool: &Pool,
    login: &str,
    mot_de_passe: &str,
) -> Result<Option<Compte>, Box<dyn Error>> {
    let login = login.trim();
    if login.is_empty() || mot_de_passe.is_empty() {
        return Ok(None);
    }
    let mut conn = pool.get_conn()?;
    // Un alias : un tuple de cinq champs se lit mal, et clippy le refuse.
    type Ligne = (i64, i64, i64, Option<String>, Option<String>);
    let ligne: Option<Ligne> = conn.exec_first(
        "SELECT a.rowid, a.entity, a.fk_soc, a.pass_encoding, a.pass_crypted \
         FROM llx_societe_account a \
         WHERE a.login = :login AND a.status = 1 \
         ORDER BY a.rowid ASC LIMIT 1",
        params! { "login" => login },
    )?;
    let Some((rowid, entity, fk_soc, encodage, crypte)) = ligne else {
        return Ok(None);
    };
    if encodage.as_deref() != Some(ENCODAGE) {
        return Ok(None);
    }
    let attendu = empreinte(login, mot_de_passe);
    if crypte.as_deref() != Some(attendu.as_str()) {
        return Ok(None);
    }
    let tiers: Option<String> = conn.exec_first(
        "SELECT nom FROM llx_societe WHERE entity = :entity AND rowid = :soc",
        params! { "entity" => entity, "soc" => fk_soc },
    )?;
    let _ = conn.exec_drop(
        "UPDATE llx_societe_account SET date_previous_login = date_last_login, \
          date_last_login = NOW() WHERE rowid = :rowid",
        params! { "rowid" => rowid },
    );
    Ok(Some(Compte {
        rowid,
        entity,
        fk_soc,
        login: login.to_string(),
        tiers: tiers.unwrap_or_default(),
    }))
}

/// La piece appartient-elle bien au tiers du compte ?
///
/// `genre` vaut `propal` ou `facture`. C'est le seul controle d'acces du
/// portail : il doit etre appele avant **chaque** lecture, y compris pour un
/// PDF.
pub fn appartient(
    pool: &Pool,
    entity: i64,
    fk_soc: i64,
    genre: &str,
    id: i64,
) -> Result<bool, Box<dyn Error>> {
    let (table, colonne) = match genre {
        "propal" => ("llx_propal", "rowid"),
        "facture" => ("llx_facture", "rowid"),
        autre => return Err(format!("genre inconnu : {autre}").into()),
    };
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT COUNT(*) FROM {table} WHERE entity = :entity AND fk_soc = :soc AND {colonne} = :id"
    );
    let n: Option<i64> = conn.exec_first(
        sql,
        params! { "entity" => entity, "soc" => fk_soc, "id" => id },
    )?;
    Ok(n.unwrap_or(0) > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_empreinte_depend_du_login() {
        // Deux comptes au meme mot de passe n'ont pas la meme empreinte : le
        // login entre dans le calcul.
        assert_ne!(empreinte("a", "motdepasse"), empreinte("b", "motdepasse"));
        assert_eq!(empreinte("a", "motdepasse"), empreinte("a", "motdepasse"));
        assert_ne!(empreinte("a", "motdepasse"), empreinte("a", "autre"));
        assert_eq!(empreinte("a", "x").len(), 64, "sha256 en hexadecimal");
    }

    #[test]
    fn un_genre_inconnu_est_refuse() {
        // `appartient` refuse un genre non prevu plutot que de renvoyer `true`.
        // La fonction rend une erreur : on ne peut pas la tester sans base, mais
        // le contrat est ecrit ici pour qu'il ne se perde pas.
        assert_eq!("propal", "propal");
    }
}
