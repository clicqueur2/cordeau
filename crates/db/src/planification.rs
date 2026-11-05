//! Planification : les taches qui tournent sans personne.
//!
//! Dolibarr a un module `cron` et une table `llx_cronjob`. Le noyau Rust n'a
//! **aucun demon** : c'est un choix, un service qui tourne en permanence pour
//! trois taches est un service de plus a surveiller, a mettre a jour et a
//! redemarrer.
//!
//! La table de Dolibarr est donc le bon endroit : un travail de type `command`
//! est **execute par le cron de Dolibarr** en lancant la ligne de commande. On y
//! depose nos taches, et les deux logiciels partagent le meme ordonnanceur —
//! ce qui regle la question du double planificateur : Dolibarr ordonnance,
//! Cordeau execute.
//!
//! Un travail qu'on ne sait pas decrire ne s'invente pas : la liste ci-dessous
//! est celle des sous-commandes que le binaire connait vraiment.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une tache programmable, telle que le noyau la connait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tache {
    /// Le nom court, qui sert aussi de sous-commande.
    pub code: &'static str,
    pub libelle: &'static str,
    /// Ce que la tache fait, en une phrase.
    pub description: &'static str,
    /// La ligne de commande a programmer.
    pub commande: &'static str,
    /// Frequence conseillee, en secondes.
    pub frequence: i64,
}

/// Les taches que le binaire sait executer (`cordeau tache <code>`).
///
/// Cette liste est **le miroir** des sous-commandes de `tache_cmd` : si l'une
/// disparait du binaire, elle disparait d'ici. Un ecran de planification qui
/// propose une commande inexistante est pire qu'aucun ecran.
pub const TACHES: &[Tache] = &[
    Tache {
        code: "relances",
        libelle: "Relances des factures en retard",
        description: "Liste les factures echues, et note un evenement par relance a envoyer.",
        commande: "cordeau tache relances --noter",
        frequence: 86_400,
    },
    Tache {
        code: "sauvegarde",
        libelle: "Sauvegarde de la base et des documents",
        description: "Dump SQL complet et dossier documentaire, dans un dossier horodate.",
        commande: "cordeau tache sauvegarde",
        frequence: 86_400,
    },
    Tache {
        code: "purge",
        libelle: "Purge des sessions et du journal",
        description: "Retire les lignes perimees des tables techniques.",
        commande: "cordeau tache purge",
        frequence: 604_800,
    },
];

/// Une tache programmee en base, telle que le cron de Dolibarr la voit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Travail {
    pub rowid: i64,
    pub libelle: String,
    pub commande: String,
    pub frequence: i64,
    pub unite: String,
    pub prochaine: Option<String>,
    pub derniere: Option<String>,
    pub resultat: Option<String>,
    pub actif: bool,
}

/// Une ligne brute de travail : `(rowid, libelle, commande, frequence, unite,
/// prochaine, derniere, resultat, statut)`.
type LigneTravail = (
    i64,
    String,
    Option<String>,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    i32,
);

/// Les travaux programmes, du plus proche au plus lointain.
pub fn travaux(pool: &Pool, entity: i64) -> Result<Vec<Travail>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneTravail> = conn.exec(
        "SELECT rowid, label, command, frequency, unitfrequency, \
                DATE_FORMAT(datenextrun, '%Y-%m-%d %H:%i'), \
                DATE_FORMAT(datelastrun, '%Y-%m-%d %H:%i'), \
                lastresult, status \
         FROM llx_cronjob \
         WHERE entity = :entity OR entity = 0 \
         ORDER BY COALESCE(datenextrun, '9999-12-31') ASC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(
            |(
                rowid,
                libelle,
                commande,
                frequence,
                unite,
                prochaine,
                derniere,
                resultat,
                statut,
            )| {
                Travail {
                    rowid,
                    libelle,
                    commande: commande.unwrap_or_default(),
                    frequence,
                    unite,
                    prochaine,
                    derniere,
                    resultat,
                    actif: statut == 1,
                }
            },
        )
        .collect())
}

/// Programme une tache dans `llx_cronjob`.
///
/// `jobtype = 'command'` : c'est le cron de Dolibarr qui lancera la ligne de
/// commande. La premiere execution est a `maintenant + frequence`.
pub fn programmer(
    pool: &Pool,
    entity: i64,
    code: &str,
    commande: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let tache = TACHES
        .iter()
        .find(|t| t.code == code)
        .ok_or_else(|| format!("tâche inconnue : « {code} »"))?;
    let commande = commande
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .unwrap_or(tache.commande);

    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_cronjob \
           (datec, jobtype, label, command, module_name, priority, datestart, datenextrun, \
            unitfrequency, frequency, maxrun, nbrun, status, processing, test, entity) \
         VALUES (NOW(), 'command', :label, :commande, 'cordeau', 0, NOW(), \
                 DATE_ADD(NOW(), INTERVAL :freq SECOND), '3600', 0, 0, 0, 1, 0, 1, :entity)",
        params! {
            "label" => tache.libelle,
            "commande" => commande,
            "freq" => tache.frequence,
            "entity" => entity,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Desactive un travail sans le supprimer : l'historique reste.
pub fn desactiver(pool: &Pool, entity: i64, rowid: i64) -> Result<usize, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_cronjob SET status = 0 WHERE rowid = :id AND (entity = :entity OR entity = 0)",
        params! { "id" => rowid, "entity" => entity },
    )?;
    Ok(conn.affected_rows() as usize)
}

/// La tache portant ce code, ou `None`.
pub fn tache(code: &str) -> Option<&'static Tache> {
    TACHES.iter().find(|t| t.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_trois_taches_sont_decrites() {
        assert_eq!(TACHES.len(), 3);
        for t in TACHES {
            assert!(!t.libelle.trim().is_empty(), "{}", t.code);
            assert!(!t.description.trim().is_empty(), "{}", t.code);
            assert!(t.commande.starts_with("cordeau tache "), "{}", t.code);
            assert!(
                t.commande.contains(t.code),
                "la commande doit porter son code"
            );
            assert!(t.frequence > 0, "{}", t.code);
        }
    }

    #[test]
    fn une_tache_se_retrouve_par_son_code() {
        assert_eq!(tache("relances").map(|t| t.code), Some("relances"));
        assert_eq!(tache("licorne"), None);
    }

    #[test]
    fn les_codes_sont_uniques() {
        let mut vus = std::collections::BTreeSet::new();
        for t in TACHES {
            assert!(vus.insert(t.code), "code en double : {}", t.code);
        }
    }
}
