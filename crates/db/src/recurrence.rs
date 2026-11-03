//! Persistance des regles de recurrence : ce que la base Dolibarr sait deja.
//!
//! Le moteur [`recurrence`] repond a « quand emettre ». Ce module repond a deux
//! autres questions : **qui** a une echeance tombee aujourd'hui, et **ce qui a
//! deja ete emis**. Il ne genere aucun document : il rend les series a honorer,
//! la regle deja relue, et il ecrit la trace d'emission une fois le document
//! cree.
//!
//! # Ce que Dolibarr stocke reellement
//!
//! Verifie colonne par colonne dans `schema-24.0.1.json` (racine du depot),
//! jamais suppose. Trois tables portent une serie, avec les memes colonnes :
//!
//! | table | colonnes de recurrence |
//! |---|---|
//! | `llx_facture_rec` | `frequency`, `unit_frequency`, `date_when`, `date_last_gen`, `nb_gen_done`, `nb_gen_max`, `suspended`, `datec` |
//! | `llx_facture_fourn_rec` | les memes, plus `ref_supplier` |
//! | `llx_fichinter_rec` | les memes sauf `suspended` (la table porte `status` a la place) |
//!
//! `date_when` (datetime) est la **prochaine** echeance, `date_last_gen` la date
//! de la derniere generation par Dolibarr, `frequency` + `unit_frequency`
//! (`varchar(2)`, defaut `'m'`) le pas. `nb_gen_done` et `nb_gen_max` bornent la
//! serie.
//!
//! Les deux tables citees dans la demande sont, elles, **muettes** :
//!
//! - `llx_contrat` n'a **aucune** colonne de recurrence (`date_contrat`,
//!   `fin_validite`, `date_cloture`, `statut` seulement). Dans Dolibarr un
//!   contrat ne se facture pas tout seul : l'utilisateur declenche « Facturer ».
//!   Un contrat alimente en revanche `llx_fichinter_rec.fk_contrat`, les
//!   interventions recurrentes.
//! - `llx_propal` n'en a pas non plus : aucune colonne `frequency`, `date_when`
//!   ou `date_last_gen`, seulement des dates de validite, de signature et
//!   `date_livraison`.
//!
//! `llx_actioncomm.recurid` / `recurrule` / `recurdateend` existent, mais ce sont
//! les rappels d'agenda : aucun document a emettre. Ils ne sont pas traites ici.
//!
//! # Le pas : ce que le moteur sait lire
//!
//! Le moteur ne connait que des pas en **mois**. Dolibarr accepte en plus le
//! jour (`'d'`) et la semaine (`'w'`), et n'importe quel multiple. Une serie
//! mensuelle de 2 mois, ou quotidienne, n'a pas d'equivalent : elle est **ecartee**
//! ([`frequence`] rend `None`) plutot que convertie au hasard. Mieux vaut ne rien
//! emettre qu'emettre au mauvais rythme.
//!
//! # La trace d'emission
//!
//! Dolibarr n'a pas de colonne « derniere emission par le noyau Rust ». Ecrire
//! `date_last_gen` a sa place serait un mensonge : Dolibarr conclurait que la
//! facture a ete generee et **sauterait** une generation.
//!
//! On ne cree pas de table (la base doit rester celle de Dolibarr, a l'octet) et
//! on n'ajoute pas d'extrafield (le declarer demanderait de toucher au modele de
//! donnees). La trace vit donc dans `llx_const`, la table de constantes de
//! Dolibarr, dont les colonnes sont `name`, `entity`, `value`, `type`, `visible`,
//! `note`, avec la cle unique `uk_const (name, entity)` :
//!
//! ```text
//! name  = DOLIBARR_RS_RECUR_<source>_<id>
//! value = AAAA-MM-JJ
//! visible = 0            (cache de l'ecran de configuration Dolibarr)
//! ```
//!
//! `<source>` est le nom logique de la table (`facture_rec`,
//! `facture_fourn_rec`, `fichinter_rec`) et non le nom SQL : la constante reste
//! lisible sans connaitre le prefixe `llx_`. La valeur est un **curseur** : elle
//! signifie « toute echeance jusqu'a cette date incluse a ete honoree ». C'est ce
//! qui rend l'emission unique : le lendemain, la prochaine echeance calculee est
//! posterieure au curseur, donc la serie n'est plus rendue.
//!
//! # Exemple
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = db::pool("mysql://root:root@127.0.0.1:3306/dolibarr")?;
//! let jour = db::aujourdhui(&pool)?;
//! for serie in db::recurrence::a_emettre(&pool, 1, &jour, 100)? {
//!     let echeance = db::recurrence::echeance(&serie)?;
//!     println!("{} {} a emettre pour le {:?}", serie.source, serie.id, echeance);
//!     // ... creation du document, puis :
//!     db::recurrence::marquer_emise(&pool, 1, &serie.source, serie.id, &jour)?;
//! }
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use recurrence::{echeances, prochaine, Frequence, Regle};

/// Prefixe des constantes de trace. Le nom complet ajoute `<source>_<id>`.
const PREFIXE_TRACE: &str = "DOLIBARR_RS_RECUR_";

/// Note posee sur la constante, pour qui la trouvera dans la base.
const NOTE_TRACE: &str = "Trace d'emission dolibarr-rs. Ne pas modifier a la main.";

/// Une source de recurrence reconnue.
struct Modele {
    /// Nom logique, repris tel quel dans la trace d'emission.
    nom: &'static str,
    /// La table Dolibarr ou vivent les series.
    table: &'static str,
    /// Condition SQL qui met une serie en pause, quand la table en a une.
    pause: Option<&'static str>,
}

/// Les trois tables de serie de Dolibarr 24.0.1.
///
/// `suspended` est nullable avec `0` pour defaut : un `NULL` ne doit pas exclure
/// la serie, d'ou le `COALESCE`.
///
/// `llx_fichinter_rec` n'a pas de colonne `suspended` : sa colonne `status`
/// (smallint, defaut 0) n'est pas un interrupteur de serie, on ne filtre pas
/// dessus plutot que d'inventer une semantique.
const MODELES: [Modele; 3] = [
    Modele {
        nom: "facture_rec",
        table: "llx_facture_rec",
        pause: Some("COALESCE(suspended, 0) = 0"),
    },
    Modele {
        nom: "facture_fourn_rec",
        table: "llx_facture_fourn_rec",
        pause: Some("COALESCE(suspended, 0) = 0"),
    },
    Modele {
        nom: "fichinter_rec",
        table: "llx_fichinter_rec",
        pause: None,
    },
];

/// Les sept colonnes lues par source, dans l'ordre du `SELECT`.
///
/// `rowid`, `date_when`, `date_last_gen`, `frequency`, `unit_frequency`,
/// `nb_gen_done`, `nb_gen_max`. Les dates sont ramenees en texte par
/// `DATE_FORMAT`, comme partout dans ce noyau.
type LigneBrute = (
    i64,
    Option<String>,
    Option<String>,
    Option<i32>,
    Option<String>,
    Option<i32>,
    Option<i32>,
);

/// Une serie dont l'echeance est tombee et qui n'a pas encore ete emise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrenceEnBase {
    /// Nom logique de la table source (`facture_rec`, ...).
    pub source: String,
    /// `rowid` de la ligne dans cette table.
    pub id: i64,
    /// La regle, prete pour [`recurrence::prochaine`] et [`recurrence::echeances`].
    ///
    /// `debut` vaut `date_when`, c'est-a-dire la prochaine echeance telle que
    /// Dolibarr la garde ; `jour` est le jour du mois de cette date, ce qui
    /// preserve les fins de mois (31 janvier -> 28 fevrier -> 31 mars).
    pub regle: Regle,
    /// Curseur d'emission : la derniere date honoree, trace confondue avec
    /// `date_last_gen`. `None` quand rien n'a jamais ete emis, et dans ce cas
    /// l'echeance due est `regle.debut`.
    pub derniere_emission: Option<String>,
    /// Reference externe de la serie, quand la table en porte une.
    ///
    /// Aucune des trois tables de recurrence n'a de colonne `ref_ext` :
    /// `llx_facture_rec` et `llx_facture_fourn_rec` ont `titre`,
    /// `llx_facture_fourn_rec` a aussi `ref_supplier`, `llx_fichinter_rec` a
    /// `title`. Le champ reste donc vide ici ; il existe pour les sources qui en
    /// porteront une un jour (`llx_contrat.ref_ext`, `llx_propal.ref_ext`), et il
    /// evite d'avoir a changer la structure ce jour-la.
    pub ref_ext: Option<String>,
}

/// La serie trouvee pour ce nom logique, ou `None`.
fn modele(nom: &str) -> Option<&'static Modele> {
    MODELES.iter().find(|modele| modele.nom == nom)
}

/// Nom de la constante de trace pour une serie.
fn nom_trace(source: &str, id: i64) -> String {
    format!("{PREFIXE_TRACE}{source}_{id}")
}

/// Jour du mois d'une date `AAAA-MM-JJ`, sans verifier qu'elle existe.
///
/// La validite du calendrier est tranchee par le moteur, pas ici.
fn jour_du_mois(date: &str) -> Option<u32> {
    let octets = date.as_bytes();
    let forme = octets.len() == 10
        && octets[4] == b'-'
        && octets[7] == b'-'
        && octets[8..].iter().all(u8::is_ascii_digit);
    if !forme {
        return None;
    }
    let jour: u32 = date.get(8..10)?.parse().ok()?;
    (1..=31).contains(&jour).then_some(jour)
}

/// Traduit `frequency` + `unit_frequency` en pas du moteur.
///
/// `unit_frequency` vaut `'d'`, `'w'`, `'m'` ou `'y'` dans Dolibarr ; le defaut
/// du schema est `'m'`. Une unite absente est donc mensuelle.
///
/// Rend `None` pour tout ce que le moteur ne sait pas exprimer : jours,
/// semaines, et multiples non prevus (2 mois, 4 mois, 2 ans). Ces series sont
/// ecartees, jamais approximees.
fn frequence(frequency: Option<i32>, unit_frequency: Option<&str>) -> Option<Frequence> {
    let pas = u32::try_from(frequency?).ok()?;
    let unite = unit_frequency.unwrap_or("m");
    match (unite, pas) {
        ("m", 1) => Some(Frequence::Mensuelle),
        ("m", 3) => Some(Frequence::Trimestrielle),
        ("m", 6) => Some(Frequence::Semestrielle),
        ("m", 12) => Some(Frequence::Annuelle),
        ("y", 1) => Some(Frequence::Annuelle),
        _ => None,
    }
}

/// Combien de documents la serie peut encore produire.
///
/// `nb_gen_max` a `0` (ou absent) veut dire « sans limite » chez Dolibarr ;
/// `nb_gen_done` compte les documents deja generes. `Some(0)` signifie que la
/// serie est epuisee et ne doit plus rien rendre.
fn limite_restante(nb_gen_max: Option<i32>, nb_gen_done: Option<i32>) -> Option<u32> {
    let maximum = u32::try_from(nb_gen_max?).ok()?;
    if maximum == 0 {
        return None;
    }
    let faites = nb_gen_done.map_or(0, |faites| u32::try_from(faites).unwrap_or(0));
    Some(maximum.saturating_sub(faites))
}

/// Verifie qu'une date `AAAA-MM-JJ` existe, en s'appuyant sur le moteur.
///
/// Le moteur est le seul juge du calendrier : on lui fait parcourir la date
/// elle-meme, il refuse `2026-02-30` comme `2026-13-01`.
fn verifier_date(date: &str) -> Result<(), Box<dyn Error>> {
    let jour = jour_du_mois(date).ok_or_else(|| format!("date invalide : {date}"))?;
    let regle = Regle {
        frequence: Frequence::Mensuelle,
        debut: date.to_string(),
        fin: None,
        limite_occurrences: None,
        jour,
    };
    echeances(&regle, date, 1)?;
    Ok(())
}

/// Toutes les traces d'emission de l'entite, indexees par nom de constante.
fn traces(
    conn: &mut mysql::PooledConn,
    entity: i64,
) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let lignes: Vec<(String, String)> = conn.exec(
        "SELECT name, value FROM llx_const \
         WHERE entity = :entity AND name LIKE 'DOLIBARR\\_RS\\_RECUR\\_%'",
        params! { "entity" => entity },
    )?;
    Ok(lignes.into_iter().collect())
}

/// Les series dont l'echeance est tombee et qui n'ont pas encore ete emises.
///
/// `aujourd_hui` est une date `AAAA-MM-JJ` ; l'echeance du jour compte comme
/// tombee. `max` borne le nombre de series rendues, toutes sources confondues.
///
/// Une serie deja marquee pour ce jour n'est plus rendue ; en revanche, appeler
/// deux fois cette fonction **sans** [`marquer_emise`] entre les deux rend deux
/// fois la meme serie. C'est voulu : la trace n'est ecrite qu'une fois le
/// document reellement cree.
///
/// # Erreurs
///
/// Date mal formee ou inexistante, et toute erreur MySQL remontee telle quelle.
pub fn a_emettre(
    pool: &Pool,
    entity: i64,
    aujourd_hui: &str,
    max: usize,
) -> Result<Vec<RecurrenceEnBase>, Box<dyn Error>> {
    if max == 0 {
        return Ok(Vec::new());
    }
    verifier_date(aujourd_hui)?;
    let mut conn = pool.get_conn()?;
    let traces = traces(&mut conn, entity)?;
    let mut out = Vec::new();

    for modele in &MODELES {
        let reste = max.saturating_sub(out.len());
        if reste == 0 {
            break;
        }
        let pause = modele
            .pause
            .map(|condition| format!(" AND {condition}"))
            .unwrap_or_default();
        // Le filtre sur `date_when` est une avancee : toute echeance calculee
        // par le moteur est posterieure ou egale a `debut`, lui-meme egal a
        // `date_when`. Le moteur tranche quand meme, plus bas.
        let sql = format!(
            "SELECT rowid, DATE_FORMAT(date_when, '%Y-%m-%d'), \
                    DATE_FORMAT(date_last_gen, '%Y-%m-%d'), \
                    frequency, unit_frequency, nb_gen_done, nb_gen_max \
             FROM {} \
             WHERE entity = :entity AND date_when IS NOT NULL AND frequency IS NOT NULL{} \
               AND DATE(date_when) <= :jour \
             ORDER BY date_when ASC, rowid ASC LIMIT {reste}",
            modele.table, pause
        );
        let lignes: Vec<LigneBrute> = conn.exec(
            sql.as_str(),
            params! { "entity" => entity, "jour" => aujourd_hui },
        )?;

        for (id, quand, dernier, frequency, unite, faites, maximum) in lignes {
            let Some(debut) = quand else { continue };
            let Some(frequence) = frequence(frequency, unite.as_deref()) else {
                continue;
            };
            let Some(jour) = jour_du_mois(&debut) else {
                continue;
            };
            let limite = limite_restante(maximum, faites);
            if limite == Some(0) {
                // Serie epuisee : Dolibarr ne produira plus rien.
                continue;
            }
            let regle = Regle {
                frequence,
                debut,
                fin: None,
                limite_occurrences: limite,
                jour,
            };

            // La trace du noyau Rust fait foi ; `date_last_gen` la complete
            // quand Dolibarr a genere lui-meme. On garde la plus recente.
            let marque = traces.get(&nom_trace(modele.nom, id)).cloned();
            let derniere = match (marque, dernier) {
                (Some(marque), Some(dernier)) => Some(marque.max(dernier)),
                (Some(marque), None) => Some(marque),
                (None, Some(dernier)) => Some(dernier),
                (None, None) => None,
            };

            let echeance = match &derniere {
                Some(derniere) => prochaine(&regle, derniere)?,
                // Rien n'a jamais ete emis : la premiere echeance est `debut`.
                None => Some(regle.debut.clone()),
            };
            let Some(echeance) = echeance else { continue };
            if echeance.as_str() > aujourd_hui {
                continue;
            }

            out.push(RecurrenceEnBase {
                source: modele.nom.to_string(),
                id,
                regle,
                derniere_emission: derniere,
                ref_ext: None,
            });
        }
    }
    Ok(out)
}

/// L'echeance due pour cette serie, telle que le moteur la calcule.
///
/// `None` quand la serie est close (derniere echeance passee, ou limite
/// d'occurrences atteinte).
///
/// # Erreurs
///
/// Les memes que [`recurrence::prochaine`] : une regle ou une date abimee.
pub fn echeance(serie: &RecurrenceEnBase) -> Result<Option<String>, recurrence::Erreur> {
    match &serie.derniere_emission {
        Some(derniere) => prochaine(&serie.regle, derniere),
        None => Ok(Some(serie.regle.debut.clone())),
    }
}

/// Ecrit la trace d'emission d'une serie, **sans toucher au document**.
///
/// La trace est une constante cachee de `llx_const` (voir l'entete du module) :
/// ni `date_when` ni `date_last_gen` ne sont reecrits, donc Dolibarr garde son
/// propre calendrier intact et le noyau Rust garde le sien.
///
/// `date` est le curseur : « toute echeance jusqu'a cette date incluse a ete
/// honoree ». En pratique c'est le jour du traitement, celui qu'on a passe a
/// [`a_emettre`].
///
/// # Erreurs
///
/// Source inconnue, identifiant nul ou negatif, date invalide, et toute erreur
/// MySQL remontee telle quelle.
pub fn marquer_emise(
    pool: &Pool,
    entity: i64,
    source: &str,
    id: i64,
    date: &str,
) -> Result<(), Box<dyn Error>> {
    if modele(source).is_none() {
        return Err(format!("source de recurrence inconnue : {source}").into());
    }
    if id <= 0 {
        return Err(format!("identifiant de recurrence invalide : {id}").into());
    }
    verifier_date(date)?;

    let nom = nom_trace(source, id);
    let mut conn = pool.get_conn()?;
    // `uk_const (name, entity)` porte la cle : une seule ligne par serie et par
    // entite, la seconde emission met la valeur a jour au lieu d'empiler. Meme
    // forme que `company::set_const` : type 'string', `visible = 0`.
    conn.exec_drop(
        "INSERT INTO llx_const (name, entity, value, type, visible, note) \
         VALUES (:nom, :entity, :valeur, 'string', 0, :note) \
         ON DUPLICATE KEY UPDATE value = VALUES(value), visible = 0, note = VALUES(note)",
        params! {
            "nom" => nom,
            "entity" => entity,
            "valeur" => date,
            "note" => NOTE_TRACE,
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_pas_mensuels_prevus_sont_reconnus() {
        assert_eq!(frequence(Some(1), Some("m")), Some(Frequence::Mensuelle));
        assert_eq!(
            frequence(Some(3), Some("m")),
            Some(Frequence::Trimestrielle)
        );
        assert_eq!(frequence(Some(6), Some("m")), Some(Frequence::Semestrielle));
        assert_eq!(frequence(Some(12), Some("m")), Some(Frequence::Annuelle));
        // Annuel s'ecrit aussi en annees chez Dolibarr.
        assert_eq!(frequence(Some(1), Some("y")), Some(Frequence::Annuelle));
        // Unite absente : le defaut du schema est 'm'.
        assert_eq!(frequence(Some(1), None), Some(Frequence::Mensuelle));
    }

    #[test]
    fn les_pas_que_le_moteur_ne_sait_pas_sont_ecartes() {
        // Jours et semaines : le moteur ne compte qu'en mois.
        assert_eq!(frequence(Some(1), Some("d")), None);
        assert_eq!(frequence(Some(2), Some("w")), None);
        // Multiples non prevus.
        assert_eq!(frequence(Some(2), Some("m")), None);
        assert_eq!(frequence(Some(4), Some("m")), None);
        assert_eq!(frequence(Some(2), Some("y")), None);
        // Pas absurdes.
        assert_eq!(frequence(Some(0), Some("m")), None);
        assert_eq!(frequence(Some(-1), Some("m")), None);
        assert_eq!(frequence(None, Some("m")), None);
    }

    #[test]
    fn la_limite_d_occurrences_retire_ce_qui_est_deja_fait() {
        assert_eq!(limite_restante(None, None), None);
        // 0 veut dire « sans limite » chez Dolibarr, pas « rien ».
        assert_eq!(limite_restante(Some(0), Some(4)), None);
        assert_eq!(limite_restante(Some(12), None), Some(12));
        assert_eq!(limite_restante(Some(12), Some(5)), Some(7));
        // Serie epuisee : plus rien a rendre.
        assert_eq!(limite_restante(Some(5), Some(5)), Some(0));
        // Coherence perdue en base : on ne rend pas de nombre negatif.
        assert_eq!(limite_restante(Some(2), Some(9)), Some(0));
    }

    #[test]
    fn le_jour_se_lit_sur_une_date_iso() {
        assert_eq!(jour_du_mois("2026-01-31"), Some(31));
        assert_eq!(jour_du_mois("2026-02-28"), Some(28));
        assert_eq!(jour_du_mois("2026-02-30"), Some(30));
        assert_eq!(jour_du_mois("2026-02-00"), None);
        assert_eq!(jour_du_mois("2026-2-8"), None);
        assert_eq!(jour_du_mois("31/01/2026"), None);
    }

    #[test]
    fn la_trace_porte_la_source_et_l_identifiant() {
        assert_eq!(
            nom_trace("facture_rec", 42),
            "DOLIBARR_RS_RECUR_facture_rec_42"
        );
        assert_eq!(
            nom_trace("fichinter_rec", 7),
            "DOLIBARR_RS_RECUR_fichinter_rec_7"
        );
    }

    #[test]
    fn une_date_qui_n_existe_pas_est_refusee() {
        assert!(verifier_date("2026-11-03").is_ok());
        assert!(verifier_date("2026-02-30").is_err());
        assert!(verifier_date("2026-13-01").is_err());
        assert!(verifier_date("2026-11-3").is_err());
        assert!(verifier_date("").is_err());
    }

    #[test]
    fn une_echeance_se_calcule_sans_marque() {
        let serie = RecurrenceEnBase {
            source: "facture_rec".to_string(),
            id: 1,
            regle: Regle {
                frequence: Frequence::Mensuelle,
                debut: "2026-01-15".to_string(),
                fin: None,
                limite_occurrences: None,
                jour: 15,
            },
            derniere_emission: None,
            ref_ext: None,
        };
        assert_eq!(echeance(&serie).unwrap(), Some("2026-01-15".to_string()));
    }
}
