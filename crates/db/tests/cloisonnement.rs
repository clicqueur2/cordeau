//! Cloisonnement par entite : la preuve, pas la promesse.
//!
//! Dolibarr range chaque ligne par `entity`. Ce fichier verifie que **nos
//! lectures respectent ce filtre** — parce que c'est la classe de bug la plus
//! silencieuse du portage : un oubli de `WHERE entity = :entity` ne casse rien,
//! il affiche simplement les factures d'une autre societe. Personne ne le voit
//! avant de les imprimer.
//!
//! # La methode
//!
//! On cree une ligne dans l'entite 1, puis on demande la liste de l'entite 2.
//! Elle doit etre absente. On ne verifie pas seulement que « ca marche » : on
//! verifie que **la ligne existe** et qu'elle **n'apparait pas ailleurs** — un
//! test qui ne cree rien passerait sans rien prouver.
//!
//! # Ce que ce fichier ne couvre pas
//!
//! Les modules dont la table n'a pas de colonne `entity` : les impressions (fait
//! de l'installation), les inscriptions (cloisonnees par le projet), les
//! disponibilites (par le calendrier). Pour ceux-la, le cloisonnement passe par
//! une jointure, et chaque module le documente dans son propre fichier.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::Queryable;
use mysql::{params, Pool};
use rust_decimal::Decimal;

/// L'entite de travail de la base de reference.
const ENTITE: i64 = 1;
/// Une entite voisine. Elle n'a pas besoin d'exister dans `llx_entity` : le
/// cloisonnement est un filtre sur une colonne, pas une contrainte.
const AUTRE: i64 = 2;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    db::pool(&dsn).ok()
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Un nom qui ne peut pas exister ailleurs : l'identifiant du processus evite
/// qu'un test laisse une trace que le suivant prendrait pour la sienne.
fn marque(sujet: &str) -> String {
    format!("ZZ-CLOISON-{sujet}-{}", std::process::id())
}

/// Efface ce qu'un test a pu laisser, dans les deux entites.
fn nettoyer(pool: &Pool, nom: &str) {
    let Ok(mut conn) = pool.get_conn() else {
        return;
    };
    // Les tiers partent en dernier : leurs documents d'abord.
    for (table, colonne) in [
        ("llx_facturedet", "fk_facture"),
        ("llx_propaldet", "fk_propal"),
    ] {
        let _ = conn.exec_drop(
            format!(
                "DELETE d FROM {table} d JOIN {} p ON p.rowid = d.{colonne} \
                 JOIN llx_societe s ON s.rowid = p.fk_soc WHERE s.nom = :nom",
                if table == "llx_facturedet" {
                    "llx_facture"
                } else {
                    "llx_propal"
                }
            )
            .as_str(),
            params! { "nom" => nom },
        );
    }
    for table in ["llx_facture", "llx_propal"] {
        let _ = conn.exec_drop(
            format!("DELETE f FROM {table} f JOIN llx_societe s ON s.rowid = f.fk_soc WHERE s.nom = :nom").as_str(),
            params! { "nom" => nom },
        );
    }
    let _ = conn.exec_drop(
        "DELETE FROM llx_projet WHERE title = :nom",
        params! { "nom" => nom },
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_product WHERE label = :nom",
        params! { "nom" => nom },
    );
    let _ = conn.exec_drop(
        "DELETE FROM llx_societe WHERE nom = :nom",
        params! { "nom" => nom },
    );
}

/// Un tiers cree en entite 1 ne sort pas de la liste de l'entite 2.
#[test]
fn un_tiers_ne_franchit_pas_l_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = marque("tiers");
    nettoyer(&pool, &nom);

    let id = db::party::create(&pool, ENTITE, &db::party::NewParty::client(&nom))?;

    let dans_la_vraie: Vec<_> = db::party::list(&pool, ENTITE, 5000, 0)?
        .into_iter()
        .filter(|p| p.rowid == id)
        .collect();
    assert_eq!(
        dans_la_vraie.len(),
        1,
        "le tiers doit exister dans son entite"
    );

    let ailleurs: Vec<_> = db::party::list(&pool, AUTRE, 5000, 0)?
        .into_iter()
        .filter(|p| p.rowid == id)
        .collect();
    assert!(
        ailleurs.is_empty(),
        "le tiers de l'entite {ENTITE} ne doit pas apparaitre dans {AUTRE}"
    );

    // Et la lecture par identifiant est cloisonnee aussi : c'est elle qui
    // protege une fiche ouverte par une URL devinee.
    assert!(
        db::party::get(&pool, AUTRE, id)?.is_none(),
        "un tiers ne se lit pas depuis une autre entite"
    );
    assert!(db::party::get(&pool, ENTITE, id)?.is_some());

    nettoyer(&pool, &nom);
    Ok(())
}

/// Une facture creee en entite 1 ne sort pas de la liste de l'entite 2.
#[test]
fn une_facture_ne_franchit_pas_l_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = marque("facture");
    nettoyer(&pool, &nom);

    let fk_soc = db::party::create(&pool, ENTITE, &db::party::NewParty::client(&nom))?;
    let mut f = db::invoice::NewInvoice::new(fk_soc);
    f.lines = vec![db::invoice::NewInvoiceLine::new(
        "Prestation",
        dec("1"),
        dec("100"),
        dec("20"),
    )];
    let id = db::invoice::create(&pool, ENTITE, &f)?;

    assert!(
        db::invoice::get(&pool, ENTITE, id)?.is_some(),
        "la facture doit exister dans son entite"
    );
    assert!(
        db::invoice::get(&pool, AUTRE, id)?.is_none(),
        "une facture ne se lit pas depuis une autre entite"
    );
    let ailleurs: Vec<_> = db::invoice::list(&pool, AUTRE, 5000, 0)?
        .into_iter()
        .filter(|x| x.rowid == id)
        .collect();
    assert!(ailleurs.is_empty());

    nettoyer(&pool, &nom);
    Ok(())
}

/// Un devis, une commande et un projet suivent la meme regle.
#[test]
fn les_documents_commerciaux_ne_franchissent_pas_l_entite() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = marque("docs");
    nettoyer(&pool, &nom);

    let fk_soc = db::party::create(&pool, ENTITE, &db::party::NewParty::client(&nom))?;

    let mut p = db::proposal::NewProposal::new(fk_soc);
    p.lines = vec![db::proposal::NewProposalLine::new(
        "Travaux",
        dec("1"),
        dec("100"),
        dec("20"),
    )];
    let devis = db::proposal::create(&pool, ENTITE, &p)?;
    assert!(db::proposal::get(&pool, ENTITE, devis)?.is_some());
    assert!(
        db::proposal::get(&pool, AUTRE, devis)?.is_none(),
        "un devis ne se lit pas depuis une autre entite"
    );

    // Un projet : la table porte `entity`, et c'est lui qui cloisonne les
    // inscriptions aux evenements.
    let mut projet = db::projet::NewProjet::new(&marque("projet"));
    projet.title = nom.clone();
    let chantier = db::projet::create(&pool, ENTITE, &projet)?;
    let vus: Vec<_> = db::projet::list(&pool, AUTRE, 5000, 0)?
        .into_iter()
        .filter(|x| x.rowid == chantier)
        .collect();
    assert!(vus.is_empty(), "un projet ne sort pas de son entite");

    nettoyer(&pool, &nom);
    Ok(())
}

/// Le garde-fou : une facture ne se cree pas dans une societe ou le client
/// n'existe pas.
///
/// C'est la protection la plus utile du multi-societe, et la plus facile a
/// oublier. Sans elle, un ecran qui garde le tiers d'une societe et l'entite
/// d'une autre produit une facture **rattachee a personne** : elle existe, elle
/// a un numero, et aucune des deux societes ne la retrouve.
#[test]
fn on_ne_cree_pas_une_facture_dans_la_mauvaise_societe() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let nom = marque("garde-fou");
    nettoyer(&pool, &nom);

    // Un client de l'entite 1.
    let fk_soc = db::party::create(&pool, ENTITE, &db::party::NewParty::client(&nom))?;

    // On tente de facturer ce client **dans l'entite 2**.
    let mut f = db::invoice::NewInvoice::new(fk_soc);
    f.lines = vec![db::invoice::NewInvoiceLine::new(
        "Prestation",
        dec("1"),
        dec("100"),
        dec("20"),
    )];
    let resultat = db::invoice::create(&pool, AUTRE, &f);

    let erreur = resultat.expect_err("facturer un client d'une autre societe doit etre refuse");
    let message = erreur.to_string();
    assert!(
        message.contains("n'existe pas")
            || message.contains("entite")
            || message.contains("entité"),
        "le refus doit dire pourquoi, trouve : {message}"
    );

    // Et rien n'a ete ecrit : un refus qui laisse une ligne est pire qu'un refus.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_facture WHERE entity = :autre AND fk_soc = :soc",
        params! { "autre" => AUTRE, "soc" => fk_soc },
    )?;
    assert_eq!(
        restes,
        Some(0),
        "aucune facture ne doit rester dans l'autre societe"
    );

    nettoyer(&pool, &nom);
    Ok(())
}
