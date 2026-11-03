//! Etat d'installation : ce qu'il faut pour que le noyau demarre, verifie.
//!
//! Dolibarr a un assistant d'installation en 1 097 fichiers. Ce noyau n'en avait
//! aucun : il fallait une base deja peuplee, des variables d'environnement et un
//! compte admin cree a la main. Cet ecran dit, point par point, ce qui manque —
//! sans rien ecrire, pour qu'on puisse le consulter sans risque.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Nombre de tables du schema Dolibarr 24.0.1 (empreinte verifiee par
/// schema verify). Une base qui en a moins n est pas installee.
pub const TABLES_ATTENDUES: i64 = 410;

/// Ce que l'installation doit avoir pour fonctionner.
#[derive(Debug, Clone)]
pub struct Etat {
    /// Nombre de tables presentes dans la base courante.
    pub tables: i64,
    /// Nombre de tables attendues par le schema Dolibarr 24.0.1.
    pub tables_attendues: i64,
    /// Utilisateurs en base, toutes entites confondues.
    pub utilisateurs: i64,
    /// Utilisateurs administrateurs (ceux qui peuvent ouvrir les reglages).
    pub admins: i64,
    /// Constantes `MAIN_MODULE_*` semees : un module non seme n'apparait pas.
    pub modules_semes: i64,
    /// Nom de la societe, s'il est renseigne.
    pub societe: Option<String>,
    /// Serveur SMTP configure, s'il l'est.
    pub smtp: Option<String>,
    /// Nombre de modules declaratifs charges (fichiers JSON).
    pub modules_declaratifs: i64,
}

impl Etat {
    /// Tout est pret : la base a son schema, il y a un administrateur, et la
    /// societe est nommee.
    pub fn pret(&self) -> bool {
        self.tables >= self.tables_attendues && self.admins > 0 && self.societe.is_some()
    }

    /// Les points qui bloquent encore, dans l'ordre ou il faut les traiter.
    pub fn manques(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.tables < self.tables_attendues {
            v.push("schema absent ou incomplet : importer schema-24.0.1.sql");
        }
        if self.admins == 0 {
            v.push("aucun administrateur : creer le premier compte");
        }
        if self.societe.is_none() {
            v.push("societe sans nom : renseigner MAIN_INFO_SOCIETE_NOM");
        }
        if self.modules_semes == 0 {
            v.push("aucun module seme : la barre de menus sera vide");
        }
        if self.smtp.is_none() {
            v.push("SMTP non configure : aucun e-mail ne partira");
        }
        v
    }
}

/// Lit l'etat de l'installation. Aucune ecriture.
pub fn etat(pool: &Pool, entity: i64, tables_attendues: i64) -> Result<Etat, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    let tables: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'",
        (),
    )?;

    let utilisateurs: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_user WHERE entity = :e",
        params! { "e" => entity },
    )?;

    let admins: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_user WHERE entity = :e AND admin = 1",
        params! { "e" => entity },
    )?;

    let modules_semes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_const WHERE name LIKE 'MAIN_MODULE\\_%'",
        (),
    )?;

    let societe: Option<String> = conn.exec_first(
        "SELECT value FROM llx_const WHERE name = 'MAIN_INFO_SOCIETE_NOM' LIMIT 1",
        (),
    )?;

    // Le SMTP peut venir de la base ou de l'environnement : les deux comptent.
    let smtp_base: Option<String> = conn.exec_first(
        "SELECT value FROM llx_const WHERE name = 'DOLIBARR_RS_SMTP_HOST' LIMIT 1",
        (),
    )?;
    let smtp = match (smtp_base, std::env::var("DOLIBARR_SMTP_HOST").ok()) {
        (Some(h), _) if !h.trim().is_empty() => Some(h),
        (_, Some(h)) if !h.trim().is_empty() => Some(h),
        _ => None,
    };

    Ok(Etat {
        tables: tables.unwrap_or(0),
        tables_attendues,
        utilisateurs: utilisateurs.unwrap_or(0),
        admins: admins.unwrap_or(0),
        modules_semes: modules_semes.unwrap_or(0),
        societe: societe.filter(|s| !s.trim().is_empty()),
        smtp,
        modules_declaratifs: 0,
    })
}
