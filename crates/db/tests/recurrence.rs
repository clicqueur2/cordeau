//! Les series Dolibarr lues par `db::recurrence` : echeance future, echeance
//! tombee une seule fois, cloisonnement d'entite.
//!
//! Tout ce qui est ecrit ici porte le prefixe `ZZ-RECUR-` et disparait dans le
//! `Drop` du garde de nettoyage, y compris quand une assertion echoue.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

const ENTITY: i64 = 1;

/// La date de reference des tests : elle ne depend pas de l'horloge de la
/// machine, sinon un test passerait en janvier et echouerait en fevrier.
const JOUR: &str = "2026-11-03";

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignore");
            None
        }
    }
}

/// Supprime la serie, sa trace et son tiers, meme si le test panique.
///
/// La base de developpement porte les cles etrangeres de Dolibarr :
/// `llx_facture_rec.fk_soc` reference `llx_societe.rowid`, d'ou le tiers.
struct Nettoyage {
    pool: Pool,
    entity: i64,
    id: i64,
    fk_soc: i64,
}

impl Nettoyage {
    /// Nom de la constante de trace, tel que `db::recurrence` l'ecrit.
    fn nom_trace(&self) -> String {
        format!("DOLIBARR_RS_RECUR_facture_rec_{}", self.id)
    }
}

impl Drop for Nettoyage {
    fn drop(&mut self) {
        let Ok(mut conn) = self.pool.get_conn() else {
            return;
        };
        let _ = conn.exec_drop(
            "DELETE FROM llx_facture_rec WHERE rowid = :id AND entity = :entity",
            params! { "id" => self.id, "entity" => self.entity },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_const WHERE name = :nom AND entity = :entity",
            params! { "nom" => self.nom_trace(), "entity" => self.entity },
        );
        let _ = conn.exec_drop(
            "DELETE FROM llx_societe WHERE rowid = :id AND entity = :entity",
            params! { "id" => self.fk_soc, "entity" => self.entity },
        );
    }
}

/// Insere un tiers de test et rend son `rowid`.
fn tiers(
    conn: &mut mysql::PooledConn,
    entity: i64,
    etiquette: &str,
) -> Result<i64, Box<dyn Error>> {
    conn.exec_drop(
        "INSERT INTO llx_societe (nom, entity, datec) VALUES (:nom, :entity, NOW())",
        params! { "nom" => etiquette, "entity" => entity },
    )?;
    conn.query_first("SELECT LAST_INSERT_ID()")?
        .ok_or_else(|| "pas d'identifiant de tiers rendu".into())
}

/// Insere une serie de facture dans `llx_facture_rec` et rend le garde.
fn serie(
    pool: &Pool,
    entity: i64,
    etiquette: &str,
    date_when: &str,
    date_last_gen: Option<&str>,
) -> Result<Nettoyage, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let fk_soc = tiers(&mut conn, entity, etiquette)?;
    conn.exec_drop(
        "INSERT INTO llx_facture_rec \
           (titre, entity, fk_soc, datec, date_when, date_last_gen, \
            frequency, unit_frequency, suspended, amount, nb_gen_done, nb_gen_max) \
         VALUES (:titre, :entity, :fk_soc, NOW(), :date_when, :date_last_gen, \
                 1, 'm', 0, 0, 0, NULL)",
        params! {
            "titre" => etiquette,
            "entity" => entity,
            "fk_soc" => fk_soc,
            "date_when" => date_when,
            "date_last_gen" => date_last_gen,
        },
    )?;
    let id: i64 = conn
        .query_first("SELECT LAST_INSERT_ID()")?
        .ok_or("pas d'identifiant rendu par l'insertion")?;
    // Un test interrompu avant son nettoyage peut avoir laisse une trace sur ce
    // rowid recycle : on repart propre.
    conn.exec_drop(
        "DELETE FROM llx_const WHERE name = :nom AND entity = :entity",
        params! { "nom" => format!("DOLIBARR_RS_RECUR_facture_rec_{id}"), "entity" => entity },
    )?;
    Ok(Nettoyage {
        pool: pool.clone(),
        entity,
        id,
        fk_soc,
    })
}

/// La serie `id` figure-t-elle dans la liste ?
fn contient(series: &[db::recurrence::RecurrenceEnBase], id: i64) -> bool {
    series.iter().any(|serie| serie.id == id)
}

/// Une echeance future ne doit rien produire : on n'emet jamais en avance.
#[test]
fn a_emettre_ne_rend_rien_quand_l_echeance_est_future() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = serie(&pool, ENTITY, "ZZ-RECUR futur", "2026-12-15", None)?;

    let series = db::recurrence::a_emettre(&pool, ENTITY, JOUR, 500)?;
    assert!(
        !contient(&series, garde.id),
        "une echeance au 15/12 rendue alors qu'on est le {JOUR}"
    );

    // La veille de l'echeance, toujours rien.
    let series = db::recurrence::a_emettre(&pool, ENTITY, "2026-12-14", 500)?;
    assert!(
        !contient(&series, garde.id),
        "la veille ne doit rien rendre"
    );

    // Le jour meme, elle tombe.
    let series = db::recurrence::a_emettre(&pool, ENTITY, "2026-12-15", 500)?;
    assert!(
        contient(&series, garde.id),
        "l'echeance du jour doit etre rendue"
    );
    Ok(())
}

/// Une echeance tombee est rendue, puis plus du tout une fois la trace ecrite ;
/// et la trace ne reecrit pas le document.
#[test]
fn a_emettre_rend_l_echeance_tombee_une_seule_fois_apres_marquage() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = serie(&pool, ENTITY, "ZZ-RECUR echue", "2026-09-15", None)?;

    let series = db::recurrence::a_emettre(&pool, ENTITY, JOUR, 500)?;
    let trouvee: Vec<_> = series.iter().filter(|serie| serie.id == garde.id).collect();
    assert_eq!(trouvee.len(), 1, "une seule ligne pour cette serie");
    let serie = trouvee[0];

    // La regle relue est celle que le moteur attend, pas une approximation.
    assert_eq!(serie.source, "facture_rec");
    assert_eq!(serie.regle.debut, "2026-09-15");
    assert_eq!(serie.regle.jour, 15);
    assert_eq!(serie.regle.frequence, recurrence::Frequence::Mensuelle);
    assert_eq!(serie.derniere_emission, None);
    assert_eq!(
        db::recurrence::echeance(serie)?,
        Some("2026-09-15".to_string())
    );

    db::recurrence::marquer_emise(&pool, ENTITY, "facture_rec", garde.id, JOUR)?;

    // Trace ecrite.
    let mut conn = pool.get_conn()?;
    let trace: Option<String> = conn.exec_first(
        "SELECT value FROM llx_const WHERE name = :nom AND entity = :entity",
        params! { "nom" => garde.nom_trace(), "entity" => ENTITY },
    )?;
    assert_eq!(trace.as_deref(), Some(JOUR), "la trace porte le curseur");

    // Le document n'a pas ete reecrit : Dolibarr garde son propre calendrier.
    let doc: Option<(String, Option<String>)> = conn.exec_first(
        "SELECT DATE_FORMAT(date_when, '%Y-%m-%d'), date_last_gen \
           FROM llx_facture_rec WHERE rowid = :id",
        params! { "id" => garde.id },
    )?;
    let (quand, dernier) = doc.ok_or("la serie a disparu")?;
    assert_eq!(quand, "2026-09-15", "date_when intacte");
    assert_eq!(dernier, None, "date_last_gen n'est pas ecrite par le noyau");

    // Deuxieme passage le meme jour : plus rien.
    let series = db::recurrence::a_emettre(&pool, ENTITY, JOUR, 500)?;
    assert!(
        !contient(&series, garde.id),
        "la serie est rendue deux fois malgre la trace"
    );

    // Le curseur se relit dans la serie, et la prochaine echeance suit la regle :
    // marquee le 03/11, la serie tombe de nouveau le 15/11, pas avant.
    let series = db::recurrence::a_emettre(&pool, ENTITY, "2026-11-15", 500)?;
    let rendue = series
        .iter()
        .find(|serie| serie.id == garde.id)
        .ok_or("l'echeance du 15/11 doit tomber")?;
    assert_eq!(rendue.derniere_emission.as_deref(), Some(JOUR));
    assert_eq!(
        db::recurrence::echeance(rendue)?,
        Some("2026-11-15".to_string())
    );
    Ok(())
}

/// Une serie que Dolibarr a deja generee (`date_last_gen`) n'est pas reemise.
#[test]
fn une_serie_deja_generee_par_dolibarr_n_est_pas_reemise() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    // Dolibarr a genere le 15/09, la prochaine echeance est le 15/12.
    let garde = serie(
        &pool,
        ENTITY,
        "ZZ-RECUR dolibarr",
        "2026-12-15",
        Some("2026-09-15"),
    )?;

    let series = db::recurrence::a_emettre(&pool, ENTITY, JOUR, 500)?;
    assert!(
        !contient(&series, garde.id),
        "Dolibarr a deja genere, le noyau ne doit pas doubler"
    );
    Ok(())
}

/// Une autre entite ne voit jamais la serie, ni sa trace.
#[test]
fn a_emettre_respecte_l_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let garde = serie(&pool, ENTITY, "ZZ-RECUR entite", "2026-09-15", None)?;

    let ici = db::recurrence::a_emettre(&pool, ENTITY, JOUR, 500)?;
    assert!(contient(&ici, garde.id), "la serie de l'entite 1 manque");

    let ailleurs = db::recurrence::a_emettre(&pool, ENTITY + 1, JOUR, 500)?;
    assert!(
        !contient(&ailleurs, garde.id),
        "la serie fuit hors de son entite"
    );

    // La trace aussi est cloisonnee : marquer ici ne marque pas ailleurs.
    db::recurrence::marquer_emise(&pool, ENTITY, "facture_rec", garde.id, JOUR)?;
    let mut conn = pool.get_conn()?;
    let trace: Option<String> = conn.exec_first(
        "SELECT value FROM llx_const WHERE name = :nom AND entity = :entity",
        params! { "nom" => garde.nom_trace(), "entity" => ENTITY + 1 },
    )?;
    assert_eq!(
        trace, None,
        "la trace ne doit pas exister dans l'autre entite"
    );
    Ok(())
}

/// Les garde-fous : source et date sont refusees plutot qu'ecrites en base.
#[test]
fn marquer_emise_refuse_une_source_ou_une_date_bidon() {
    let Some(pool) = pool_or_skip() else {
        return;
    };
    assert!(
        db::recurrence::marquer_emise(&pool, ENTITY, "table_inconnue", 1, JOUR).is_err(),
        "source inconnue acceptee"
    );
    assert!(
        db::recurrence::marquer_emise(&pool, ENTITY, "facture_rec", 0, JOUR).is_err(),
        "identifiant nul accepte"
    );
    assert!(
        db::recurrence::marquer_emise(&pool, ENTITY, "facture_rec", 1, "2026-02-30").is_err(),
        "date impossible acceptee"
    );
    assert!(
        db::recurrence::a_emettre(&pool, ENTITY, "03/11/2026", 10).is_err(),
        "date mal formee acceptee"
    );
    assert!(
        db::recurrence::a_emettre(&pool, ENTITY, JOUR, 0)
            .expect("maximum nul n'est pas une erreur")
            .is_empty(),
        "un maximum nul rend une liste vide"
    );
}
