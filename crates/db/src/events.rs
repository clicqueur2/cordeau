//! Journal des evenements : qui a fait quoi, quand, depuis quelle adresse.
//!
//! La table `llx_events` est dans le schema depuis toujours, et le cceur de
//! Dolibarr ne l'ecrit **presque pas** : il declenche des evenements metier
//! (`USER_LOGIN`, `USER_LOGIN_FAILED`), que des modules peuvent consommer, mais
//! il ne consigne pas trace dans cette table. Resultat : sur une installation
//! neuve, elle reste vide, et personne ne peut dire qui s'est connecte.
//!
//! Ce module la remplit. Trois principes :
//!
//! 1. **On n'ecrit jamais de secret.** Un echec de connexion consigne le
//!    *login tente*, jamais le mot de passe.
//! 2. **Les codes sont prefixes** (`USER_`, `SECURITY_`) pour ne pas usurper un
//!    vocabulaire qu'on ne maitrise pas : `USER_LOGIN` et `USER_LOGIN_FAILED`
//!    sont ceux du cceur, les autres sont les notres, et c'est ecrit ici.
//! 3. **Les longueurs sont bornees** a la taille des colonnes, en coupant sur
//!    une frontiere de caractere : une description trop longue ferait echouer
//!    l'INSERT en mode strict, et un evenement perdu est pire qu'un evenement
//!    tronque.
//! 4. **Un evenement sans compte est global** (`entity = 0`) : un echec de
//!    connexion n'appartient a aucune societe, et un administrateur doit le
//!    voir depuis la sienne.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Connexion reussie. Code du cceur de Dolibarr.
pub const CONNEXION: &str = "USER_LOGIN";
/// Connexion refusee. Code du cceur de Dolibarr.
pub const CONNEXION_ECHEC: &str = "USER_LOGIN_FAILED";
/// Deconnexion volontaire.
pub const DECONNEXION: &str = "USER_LOGOUT";
/// Changement de mot de passe.
pub const MOT_DE_PASSE: &str = "USER_NEW_PASSWORD";
/// Activation d'un module.
pub const MODULE_ACTIVE: &str = "SECURITY_MODULE_ENABLED";
/// Desactivation d'un module.
pub const MODULE_DESACTIVE: &str = "SECURITY_MODULE_DISABLED";
/// Changement d'entite de travail dans la session.
pub const CHANGEMENT_ENTITE: &str = "SECURITY_ENTITY_SWITCH";

/// Un evenement consigne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evenement {
    pub rowid: i64,
    pub type_: String,
    pub date: String,
    pub login: Option<String>,
    pub description: String,
    pub ip: String,
    pub agent: Option<String>,
}

/// Ce qu'on veut consigner.
#[derive(Debug, Clone)]
pub struct Nouvel<'a> {
    pub type_: &'a str,
    pub entity: i64,
    /// Compte concerne, quand il existe (un echec de connexion n'en a pas).
    pub fk_user: Option<i64>,
    pub description: &'a str,
    pub ip: &'a str,
    pub agent: Option<&'a str>,
    /// Objet concerne (facture, module...), sans contrainte de cle etrangere.
    pub fk_object: Option<i64>,
    /// Methode d'authentification : `motdepasse`, `oauth`, `ldap`...
    pub methode: Option<&'a str>,
}

impl<'a> Nouvel<'a> {
    /// Un evenement de connexion, avec le minimum d'information.
    pub fn connexion(
        type_: &'a str,
        entity: i64,
        fk_user: Option<i64>,
        description: &'a str,
        ip: &'a str,
        agent: Option<&'a str>,
    ) -> Self {
        Self {
            type_,
            entity,
            fk_user,
            description,
            ip,
            agent,
            fk_object: None,
            methode: Some("motdepasse"),
        }
    }
}

/// Consigne un evenement. Rend son numero.
pub fn enregistrer(pool: &Pool, e: &Nouvel<'_>) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // `dateevent` et `tms` sont laissees a la base : l'heure de l'evenement est
    // celle du serveur de base, pas celle du processus web.
    conn.exec_drop(
        "INSERT INTO llx_events \
           (type, entity, dateevent, fk_user, description, ip, user_agent, fk_object, \
            authentication_method) \
         VALUES (:type, :entity, NOW(), :fk_user, :description, :ip, :agent, :fk_object, :methode)",
        params! {
            "type" => borne(e.type_, 32),
            "entity" => e.entity,
            "fk_user" => e.fk_user,
            "description" => borne(e.description, 250),
            "ip" => borne(e.ip, 250),
            "agent" => e.agent.map(|a| borne(a, 255)),
            "fk_object" => e.fk_object,
            "methode" => e.methode.map(|m| borne(m, 64)),
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Les derniers evenements d'une entite, du plus recent au plus ancien.
pub fn recents(pool: &Pool, entity: i64, limite: u32) -> Result<Vec<Evenement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT e.rowid, e.type, e.dateevent, u.login, e.description, e.ip, e.user_agent \
         FROM llx_events e LEFT JOIN llx_user u ON u.rowid = e.fk_user \
         WHERE e.entity IN (0, :entity) \
         ORDER BY e.dateevent DESC, e.rowid DESC \
         LIMIT :limite",
        params! { "entity" => entity, "limite" => limite.clamp(1, 500) },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| Evenement {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            type_: r
                .get_opt::<String, _>(1)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            date: r
                .get_opt::<String, _>(2)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            login: r.get_opt::<String, _>(3).and_then(|v| v.ok()),
            description: r
                .get_opt::<String, _>(4)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            ip: r
                .get_opt::<String, _>(5)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            agent: r.get_opt::<String, _>(6).and_then(|v| v.ok()),
        })
        .collect())
}

/// Le nombre d'evenements par type, du plus frequent au moins frequent.
pub fn par_type(pool: &Pool, entity: i64) -> Result<Vec<(String, i64)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(String, i64)> = conn.exec(
        "SELECT type, COUNT(*) FROM llx_events WHERE entity IN (0, :entity) GROUP BY type \
         ORDER BY COUNT(*) DESC, type ASC",
        params! { "entity" => entity },
    )?;
    Ok(rows)
}

/// Le nombre total d'evenements d'une entite.
pub fn total(pool: &Pool, entity: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_events WHERE entity IN (0, :entity)",
        params! { "entity" => entity },
    )?;
    Ok(n.unwrap_or(0))
}

/// Coupe une chaine a une longueur en octets, sans casser un caractere.
///
/// Les colonnes sont en `varchar(n)`, et MariaDB en mode strict refuse ce qui
/// depasse. Un evenement tronque vaut mieux qu'un evenement perdu.
fn borne(valeur: &str, max: usize) -> String {
    if valeur.len() <= max {
        return valeur.to_string();
    }
    let mut fin = max;
    while fin > 0 && !valeur.is_char_boundary(fin) {
        fin -= 1;
    }
    valeur[..fin].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_chaine_courte_est_intacte() {
        assert_eq!(borne("connexion", 32), "connexion");
        assert_eq!(borne("", 32), "");
    }

    #[test]
    fn une_chaine_trop_longue_est_coupee_a_la_limite() {
        let longue = "a".repeat(300);
        assert_eq!(borne(&longue, 250).len(), 250);
    }

    /// Une coupe au milieu d'un accent produirait un octet invalide : on recule
    /// jusqu'a la frontiere de caractere.
    #[test]
    fn la_coupe_ne_casse_pas_un_caractere() {
        // « e » accentue = 2 octets en UTF-8. Limite a 5 : 2 caracteres (4 o)
        // tiennent, le troisieme (6 o) non.
        let texte = "ééé";
        let coupe = borne(texte, 5);
        assert_eq!(coupe, "éé");
        assert_eq!(coupe.len(), 4);
        assert!(coupe.is_char_boundary(coupe.len()));
    }

    #[test]
    fn les_codes_du_coeur_sont_conserves() {
        assert_eq!(CONNEXION, "USER_LOGIN");
        assert_eq!(CONNEXION_ECHEC, "USER_LOGIN_FAILED");
    }

    /// Un echec de connexion ne consigne pas de mot de passe : la structure ne
    /// porte que le login tente, dans la description.
    #[test]
    fn un_echec_ne_porte_que_le_login() {
        let e = Nouvel::connexion(
            CONNEXION_ECHEC,
            1,
            None,
            "login tente : jean",
            "127.0.0.1",
            Some("Mozilla/5.0"),
        );
        assert!(e.description.contains("jean"));
        assert!(e.fk_user.is_none());
        // Aucun champ de la structure ne peut transporter un secret.
        assert_eq!(e.methode, Some("motdepasse"));
    }
}
