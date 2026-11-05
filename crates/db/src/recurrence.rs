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
/// **Tout ce que Dolibarr accepte est traduit.** Une unite inconnue ou un pas
/// nul rend `None`, et la serie est ecartee : mieux vaut ne rien emettre qu'un
/// document a une date inventee. Mais ecarter `'d'`, `'w'` et les pas libres
/// (2 mois, 4 mois, 2 ans) revenait a faire disparaitre en silence les series
/// quotidiennes et hebdomadaires, qui sont les plus courantes en entretien.
fn frequence(frequency: Option<i32>, unit_frequency: Option<&str>) -> Option<Frequence> {
    let pas = u32::try_from(frequency?).ok()?;
    if pas == 0 {
        return None;
    }
    let unite = unit_frequency.unwrap_or("m");
    match unite {
        "d" => Some(Frequence::Jours(pas)),
        "w" => Some(Frequence::Semaines(pas)),
        "m" => Some(mois(pas)),
        // Douze mois font un an : on ramene pour que le pas reste en mois.
        "y" => pas.checked_mul(12).map(mois),
        _ => None,
    }
}

/// Le pas en mois, avec les presets de l'interface Dolibarr nommes.
///
/// `Mois(1)` et `Mensuelle` calculent pareil ; garder les variantes nommees
/// evite qu'une regle relue en base porte une forme que l'ecran ne produit
/// jamais, et qu'un test de coherence compare deux choses identiques.
fn mois(pas: u32) -> Frequence {
    match pas {
        1 => Frequence::Mensuelle,
        3 => Frequence::Trimestrielle,
        6 => Frequence::Semestrielle,
        12 => Frequence::Annuelle,
        n => Frequence::Mois(n),
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

/// L'en-tete d'une facture, tel qu'on le recopie dans la serie :
/// `(tiers, ht, tva, ttc, condition, compte, auteur)`.
type EnteteFacture = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    i64,
);

/// Cree une serie de factures recurrentes a partir d'une facture existante.
///
/// Dolibarr garde le modele dans `llx_facture_rec` (en-tete) et
/// `llx_facturedet_rec` (lignes) : c'est une **copie** de la facture, pas un
/// lien vers elle. Modifier la facture d'origine ne change donc rien a la serie,
/// et c'est voulu — une mensualite ne doit pas bouger parce qu'on a corrige la
/// premiere.
///
/// `unite` : `d` jour, `w` semaine, `m` mois, `y` annee. `frequence` est le
/// nombre d'unites entre deux emissions (1 = tous les mois, 3 = tous les
/// trimestres). `maximum` a 0 = sans fin.
pub fn depuis_facture(
    pool: &Pool,
    entity: i64,
    fk_facture: i64,
    titre: &str,
    prochaine: &str,
    periodicite: (i32, &str),
    maximum: i32,
) -> Result<i64, Box<dyn Error>> {
    let (frequence, unite) = periodicite;
    if frequence <= 0 {
        return Err("la frequence doit etre superieure a zero".into());
    }
    if !matches!(unite, "d" | "w" | "m" | "y") {
        return Err(format!("unite de frequence inconnue : {unite}").into());
    }
    if prochaine.len() != 10 {
        return Err("la prochaine echeance doit etre au format AAAA-MM-JJ".into());
    }
    let titre = if titre.trim().is_empty() {
        "Serie de factures".to_string()
    } else {
        titre.trim().to_string()
    };

    let mut conn = pool.get_conn()?;
    let entete: Option<EnteteFacture> = conn.exec_first(
        "SELECT fk_soc, CAST(total_ht AS CHAR), CAST(total_tva AS CHAR), \
                CAST(total_ttc AS CHAR), fk_cond_reglement, fk_account, fk_user_author \
         FROM llx_facture WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => fk_facture },
    )?;
    let Some((fk_soc, ht, tva, ttc, cond, compte, auteur)) = entete else {
        return Err(format!("facture {fk_facture} introuvable").into());
    };

    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    tx.exec_drop(
        "INSERT INTO llx_facture_rec \
           (titre, entity, fk_soc, datec, amount, total_ht, total_tva, total_ttc, \
            fk_cond_reglement, fk_account, date_when, frequency, unit_frequency, \
            nb_gen_done, nb_gen_max, fk_user_author) \
         VALUES (:titre, :entity, :soc, NOW(), :ttc, :ht, :tva, :ttc, \
                 :cond, :compte, :prochaine, :freq, :unite, 0, :max, :auteur)",
        params! {
            "titre" => titre,
            "entity" => entity,
            "soc" => fk_soc,
            "ht" => ht.unwrap_or_else(|| "0".into()),
            "tva" => tva.unwrap_or_else(|| "0".into()),
            "ttc" => ttc.unwrap_or_else(|| "0".into()),
            // `fk_cond_reglement` est NOT NULL sans defaut : 0 veut dire
            // « non precise », ce qui vaut mieux qu'une valeur inventee.
            "cond" => cond.unwrap_or(0),
            "compte" => compte,
            "prochaine" => format!("{prochaine} 00:00:00"),
            "freq" => frequence,
            "unite" => unite,
            "max" => maximum.max(0),
            // L'auteur de la serie est celui de la facture : la colonne porte une
            // cle etrangere, et 0 y est refuse.
            "auteur" => auteur,
        },
    )?;
    let id = tx.last_insert_id().unwrap_or(0) as i64;

    // Les lignes, telles quelles. Le modele est une copie figee.
    tx.exec_drop(
        "INSERT INTO llx_facturedet_rec \
           (fk_facture, fk_product, product_type, description, subprice, tva_tx, \
            qty, total_ht, total_tva, total_ttc, fk_unit) \
         SELECT :id, fk_product, product_type, description, subprice, tva_tx, \
                qty, total_ht, total_tva, total_ttc, fk_unit \
         FROM llx_facturedet WHERE fk_facture = :source",
        params! { "id" => id, "source" => fk_facture },
    )?;

    // Une facture sans ligne ne ferait pas une serie utile.
    let n: Option<i64> = tx.exec_first(
        "SELECT COUNT(*) FROM llx_facturedet_rec WHERE fk_facture = :id",
        params! { "id" => id },
    )?;
    if n.unwrap_or(0) == 0 {
        return Err("la facture n'a aucune ligne : rien a recopier".into());
    }

    tx.commit()?;
    Ok(id)
}

#[cfg(test)]
mod tests_serie {
    use super::*;

    const ENTITE: i64 = 999;

    fn pool() -> Option<Pool> {
        let dsn = std::env::var("DOLIBARR_DSN").ok()?;
        match crate::pool(&dsn) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("connexion impossible ({e}) : test ignore");
                None
            }
        }
    }

    fn nettoyer(pool: &Pool) {
        let mut conn = pool.get_conn().expect("connexion");
        let _ = conn.exec_drop(
            "DELETE FROM llx_facturedet_rec WHERE fk_facture IN \
               (SELECT rowid FROM (SELECT rowid FROM llx_facture_rec WHERE entity = :e) x)",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_facture_rec WHERE entity = :e",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_facturedet WHERE fk_facture IN \
               (SELECT rowid FROM (SELECT rowid FROM llx_facture WHERE entity = :e) y)",
            params! { "e" => ENTITE },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_facture WHERE entity = :e",
            params! { "e" => ENTITE },
        );
    }

    fn facture(pool: &Pool) -> i64 {
        let mut conn = pool.get_conn().expect("connexion");
        let soc: i64 = conn
            .query_first("SELECT rowid FROM llx_societe ORDER BY rowid LIMIT 1")
            .expect("requete tiers")
            .expect("aucun tiers");
        let usr: i64 = conn
            .query_first("SELECT rowid FROM llx_user ORDER BY rowid LIMIT 1")
            .expect("requete utilisateur")
            .expect("aucun utilisateur");
        conn.exec_drop(
            "INSERT INTO llx_facture (ref, entity, fk_soc, fk_user_author, datec, datef, \
                total_ht, total_tva, total_ttc, fk_statut, paye) \
             VALUES ('RECETTE-REC-1', :e, :soc, :usr, NOW(), NOW(), 100, 20, 120, 1, 0)",
            params! { "e" => ENTITE, "soc" => soc, "usr" => usr },
        )
        .expect("facture");
        let id = conn.last_insert_id() as i64;
        conn.exec_drop(
            "INSERT INTO llx_facturedet (fk_facture, product_type, description, subprice, \
                subprice_ttc, tva_tx, qty, total_ht, total_tva, total_ttc) \
             VALUES (:id, 0, 'Entretien mensuel', 100, 120, 20, 1, 100, 20, 120)",
            params! { "id" => id },
        )
        .expect("ligne");
        id
    }

    #[test]
    fn une_serie_recopie_l_entete_et_les_lignes() {
        let Some(p) = pool() else { return };
        nettoyer(&p);
        let source = facture(&p);

        let id = depuis_facture(&p, ENTITE, source, "Entretien", "2026-12-01", (1, "m"), 12)
            .expect("creation de la serie");

        let mut conn = p.get_conn().expect("connexion");
        let tete: (String, String, i32, i32, String) = conn
            .exec_first(
                "SELECT titre, CAST(total_ttc AS CHAR), frequency, nb_gen_max, unit_frequency \
                 FROM llx_facture_rec WHERE rowid = :id",
                params! { "id" => id },
            )
            .unwrap()
            .expect("la serie");
        assert_eq!(tete.0, "Entretien");
        assert_eq!(tete.1.parse::<f64>().unwrap(), 120.0);
        assert_eq!(tete.2, 1);
        assert_eq!(tete.3, 12);
        assert_eq!(tete.4, "m");

        let lignes: i64 = conn
            .exec_first(
                "SELECT COUNT(*) FROM llx_facturedet_rec WHERE fk_facture = :id",
                params! { "id" => id },
            )
            .unwrap()
            .unwrap();
        assert_eq!(lignes, 1, "la ligne est recopiee");

        // Modifier la facture d'origine ne touche pas la serie : c'est une copie.
        conn.exec_drop(
            "UPDATE llx_facture SET total_ttc = 999 WHERE rowid = :id",
            params! { "id" => source },
        )
        .unwrap();
        let apres: String = conn
            .exec_first(
                "SELECT CAST(total_ttc AS CHAR) FROM llx_facture_rec WHERE rowid = :id",
                params! { "id" => id },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            apres.parse::<f64>().unwrap(),
            120.0,
            "la serie garde le montant d'origine"
        );

        nettoyer(&p);
    }

    #[test]
    fn une_facture_sans_ligne_est_refusee() {
        let Some(p) = pool() else { return };
        nettoyer(&p);
        let mut conn = p.get_conn().expect("connexion");
        let soc: i64 = conn
            .query_first("SELECT rowid FROM llx_societe ORDER BY rowid LIMIT 1")
            .unwrap()
            .unwrap();
        let usr: i64 = conn
            .query_first("SELECT rowid FROM llx_user ORDER BY rowid LIMIT 1")
            .unwrap()
            .unwrap();
        conn.exec_drop(
            "INSERT INTO llx_facture (ref, entity, fk_soc, fk_user_author, datec, datef, \
                total_ht, total_tva, total_ttc, fk_statut, paye) \
             VALUES ('RECETTE-REC-VIDE', :e, :soc, :usr, NOW(), NOW(), 0, 0, 0, 1, 0)",
            params! { "e" => ENTITE, "soc" => soc, "usr" => usr },
        )
        .unwrap();
        let id = conn.last_insert_id() as i64;

        assert!(depuis_facture(&p, ENTITE, id, "", "2026-12-01", (1, "m"), 0).is_err());
        nettoyer(&p);
    }

    #[test]
    fn une_frequence_absurde_est_refusee() {
        let Some(p) = pool() else { return };
        nettoyer(&p);
        let source = facture(&p);
        assert!(depuis_facture(&p, ENTITE, source, "", "2026-12-01", (0, "m"), 0).is_err());
        assert!(depuis_facture(&p, ENTITE, source, "", "2026-12-01", (1, "x"), 0).is_err());
        assert!(depuis_facture(&p, ENTITE, source, "", "2026-12", (1, "m"), 0).is_err());
        assert!(depuis_facture(&p, ENTITE, 999_999, "", "2026-12-01", (1, "m"), 0).is_err());
        nettoyer(&p);
    }
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
    fn les_jours_et_les_semaines_sont_traduits() {
        // Le formulaire Dolibarr accepte d/w/m/y. Une serie quotidienne ou
        // hebdomadaire ecartee en silence ne serait jamais facturee.
        assert_eq!(frequence(Some(1), Some("d")), Some(Frequence::Jours(1)));
        assert_eq!(frequence(Some(15), Some("d")), Some(Frequence::Jours(15)));
        assert_eq!(frequence(Some(2), Some("w")), Some(Frequence::Semaines(2)));
    }

    #[test]
    fn les_multiples_libres_en_mois_sont_traduits() {
        // Deux mois, quatre mois, deux ans : saisissables dans l'interface.
        assert_eq!(frequence(Some(2), Some("m")), Some(Frequence::Mois(2)));
        assert_eq!(frequence(Some(4), Some("m")), Some(Frequence::Mois(4)));
        assert_eq!(frequence(Some(2), Some("y")), Some(Frequence::Mois(24)));
    }

    #[test]
    fn les_pas_absurdes_sont_ecartes() {
        // Un pas nul boucherait la serie a l'infini ; une unite inconnue
        // n'a pas de sens traduit.
        assert_eq!(frequence(Some(0), Some("m")), None);
        assert_eq!(frequence(Some(-1), Some("m")), None);
        assert_eq!(frequence(None, Some("m")), None);
        assert_eq!(frequence(Some(1), Some("z")), None);
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
