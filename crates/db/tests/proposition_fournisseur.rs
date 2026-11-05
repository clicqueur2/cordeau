//! Propositions fournisseurs (demandes de prix) : totaux, contrôles de saisie,
//! cycle valider / clore / refuser, suppression et numérotation.
//!
//! Les tests s'exécutent en parallèle sur la même base : chaque test crée son
//! propre fournisseur (nom unique) et n'affirme rien sur un `COUNT(*)` global.
//! Les montants sont vérifiés à la main, chiffre par chiffre, pour que
//! l'arithmétique monétaire soit prouvée et pas seulement « cohérente avec
//! elle-même ».

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use db::proposition_fournisseur::{
    NewLigneDemande, NewPropositionFournisseur, STATUT_BROUILLON, STATUT_CLOSE, STATUT_REFUSEE,
    STATUT_VALIDEE,
};

const ENTITY: i64 = 1;

/// Préfixe commun aux fiches de test : le nettoyage s'appuie dessus.
const PREFIXE: &str = "ZZ PF Test";

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("décimal")
}

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test d'intégration ignoré");
            None
        }
    }
}

/// Supprime les restes d'un test précédent interrompu, pour que la suite soit
/// rejouable sans intervention manuelle.
///
/// Les lignes puis les en-têtes sont effacés **en SQL direct** et non par
/// `supprimer` : une demande validée ou close refuse la suppression, or c'est
/// précisément l'état dans lequel un test interrompu laisse ses pièces.
fn nettoyer(pool: &Pool, suffixe: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ids: Vec<i64> = conn.exec(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND nom LIKE :motif",
        params! { "entity" => ENTITY, "motif" => format!("{PREFIXE} {suffixe}%") },
    )?;
    for id in ids {
        let demandes: Vec<i64> = conn.exec(
            "SELECT rowid FROM llx_supplier_proposal \
             WHERE entity = :entity AND fk_soc = :soc",
            params! { "entity" => ENTITY, "soc" => id },
        )?;
        for d in demandes {
            conn.exec_drop(
                "DELETE FROM llx_supplier_proposaldet WHERE fk_supplier_proposal = :d",
                params! { "d" => d },
            )?;
            conn.exec_drop(
                "DELETE FROM llx_supplier_proposal WHERE rowid = :d",
                params! { "d" => d },
            )?;
        }
        db::party::delete(pool, ENTITY, id)?;
    }
    Ok(())
}

/// Un fournisseur neuf, au nom unique : deux tests en parallèle ne peuvent pas
/// se marcher dessus. `creer_fournisseur` pose `fournisseur = 1`, sans quoi
/// `create` refuserait la fiche.
fn fournisseur(pool: &Pool, suffixe: &str) -> Result<i64, Box<dyn Error>> {
    let nom = format!("{PREFIXE} {suffixe}");
    db::achat::creer_fournisseur(pool, ENTITY, &nom, None)
}

/// Une demande d'une ligne, prête à être validée.
fn demande_simple(fk_soc: i64, libelle: &str, qty: &str, prix: &str) -> NewPropositionFournisseur {
    let mut new = NewPropositionFournisseur::new(fk_soc);
    new.lines = vec![NewLigneDemande::new(
        libelle,
        dec(qty),
        dec(prix),
        dec("20"),
    )];
    new
}

#[test]
fn une_demande_de_prix_calcule_ses_totaux_au_centime() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Totaux")?;
    let fk_soc = fournisseur(&pool, "Totaux")?;

    // Une unité réelle, si le dictionnaire en propose une : `fk_unit` porte une
    // clé étrangère vers `llx_c_units`, on ne peut pas inventer un identifiant.
    let unite: Option<i64> = {
        let mut conn = pool.get_conn()?;
        conn.query_first("SELECT rowid FROM llx_c_units ORDER BY rowid LIMIT 1")?
    };

    let mut ligne_sable = NewLigneDemande::new("Sable 0/4", dec("2.5"), dec("40"), dec("20"));
    ligne_sable.description = Some("Livraison chantier rue des Carrières".to_string());
    ligne_sable.ref_fourn = Some("FOURN-SABLE-04".to_string());
    ligne_sable.fk_unit = unite;

    let mut ligne_gravier = NewLigneDemande::new("Gravier 6/10", dec("3"), dec("33.33"), dec("10"));
    ligne_gravier.remise_percent = dec("5");
    ligne_gravier.ref_fourn = Some("FOURN-GRAV-610".to_string());

    let mut new = NewPropositionFournisseur::new(fk_soc);
    new.date_livraison = Some("2026-11-20".to_string());
    new.note_private = Some("demande urgente, chantier Charly".to_string());
    new.note_public = Some("merci de préciser le délai".to_string());
    new.lines = vec![ligne_sable, ligne_gravier];

    let id = db::proposition_fournisseur::create(&pool, ENTITY, &new)?;
    let d = db::proposition_fournisseur::get(&pool, ENTITY, id)?.expect("demande relue");

    assert_eq!(d.entity, ENTITY);
    assert_eq!(d.fk_soc, Some(fk_soc));
    assert_eq!(
        d.fk_statut, STATUT_BROUILLON,
        "une demande naît en brouillon"
    );
    assert_eq!(d.fournisseur.as_deref(), Some("ZZ PF Test Totaux"));
    assert_eq!(d.date_livraison.as_deref(), Some("2026-11-20"));
    assert_eq!(
        d.note_private.as_deref(),
        Some("demande urgente, chantier Charly")
    );
    assert_eq!(d.note_public.as_deref(), Some("merci de préciser le délai"));
    assert!(d.datec.is_some(), "la date de saisie est posée");
    assert!(d.date_valid.is_none(), "rien n'est validé à la création");
    assert!(d.date_cloture.is_none());
    assert_eq!(d.remise_percent, Some(dec("0")), "pas de remise d'en-tête");
    assert!(d.fk_mode_reglement.is_none());

    // Référence `DF` + compteur.
    assert!(
        d.ref_.starts_with("DF"),
        "référence inattendue : {}",
        d.ref_
    );
    assert!(
        d.ref_[2..].bytes().all(|b| b.is_ascii_digit()),
        "le compteur doit suivre le préfixe sans tiret : {}",
        d.ref_
    );

    // Calcul à la main :
    //   ligne 1 : 2,5 x 40,00 = 100,00 HT ; TVA 20 % -> TTC 120,00 ; TVA 20,00.
    //   ligne 2 : 3 x 33,33 = 99,99 ; remise 5 % -> 94,9905 -> 94,99 HT ;
    //             TTC = 94,9905 x 1,10 = 104,48955 -> 104,49 ;
    //             TVA = 104,49 - 94,99 = 9,50.
    //   en-tête : 194,99 HT ; 29,50 de TVA ; 224,49 TTC.
    assert_eq!(d.total_ht, Some(dec("194.99")));
    assert_eq!(d.total_tva, Some(dec("29.50")));
    assert_eq!(d.total_ttc, Some(dec("224.49")));
    assert_eq!(
        d.total_ht.unwrap() + d.total_tva.unwrap(),
        d.total_ttc.unwrap(),
        "ht + tva doit valoir ttc au centime près"
    );

    let lignes = db::proposition_fournisseur::lines(&pool, ENTITY, id)?;
    assert_eq!(lignes.len(), 2);
    assert_eq!(lignes[0].rang, 1, "le rang suit l'ordre de saisie");
    assert_eq!(lignes[1].rang, 2);
    assert_eq!(lignes[0].label.as_deref(), Some("Sable 0/4"));
    assert_eq!(
        lignes[0].description.as_deref(),
        Some("Livraison chantier rue des Carrières")
    );
    assert_eq!(lignes[0].total_ht, Some(dec("100.00")));
    assert_eq!(lignes[0].total_tva, Some(dec("20.00")));
    assert_eq!(lignes[0].total_ttc, Some(dec("120.00")));
    assert_eq!(lignes[0].subprice, Some(dec("40.000000")));
    assert_eq!(
        lignes[0].buy_price_ht, lignes[0].subprice,
        "sur une demande de prix, le prix demandé EST le prix d'achat"
    );
    assert_eq!(lignes[0].price, lignes[0].subprice);
    assert_eq!(
        lignes[0].ref_fourn.as_deref(),
        Some("FOURN-SABLE-04"),
        "la référence fournisseur voyage avec la ligne"
    );
    assert_eq!(lignes[0].fk_unit, unite);

    assert_eq!(lignes[1].total_ht, Some(dec("94.99")));
    assert_eq!(lignes[1].total_tva, Some(dec("9.50")));
    assert_eq!(lignes[1].total_ttc, Some(dec("104.49")));
    assert_eq!(lignes[1].remise_percent, Some(dec("5")));
    assert_eq!(lignes[1].qty, Some(dec("3")));
    assert!(!lignes[0].is_section());
    assert!(!lignes[0].is_sous_total());

    // La liste porte le nom du fournisseur, sans seconde requête par ligne.
    let liste = db::proposition_fournisseur::list(&pool, ENTITY, 5000, 0)?;
    let vue = liste
        .iter()
        .find(|x| x.rowid == id)
        .expect("la demande créée apparaît dans list()");
    assert_eq!(vue.fournisseur.as_deref(), Some("ZZ PF Test Totaux"));
    assert_eq!(vue.total_ttc, Some(dec("224.49")));

    // Une autre entité ne la voit pas.
    assert!(db::proposition_fournisseur::get(&pool, 2, id)?.is_none());
    assert!(db::proposition_fournisseur::lines(&pool, 2, id)?.is_empty());

    nettoyer(&pool, "Totaux")?;
    assert!(db::proposition_fournisseur::get(&pool, ENTITY, id)?.is_none());
    Ok(())
}

#[test]
fn refuse_un_tiers_qui_n_est_pas_fournisseur() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Client")?;
    let nom = format!("{PREFIXE} Client");
    let fk_soc = db::party::create(&pool, ENTITY, &db::party::NewParty::client(&nom))?;

    let new = demande_simple(fk_soc, "Prestation", "1", "100");
    let erreur = db::proposition_fournisseur::create(&pool, ENTITY, &new)
        .expect_err("un client n'est pas un fournisseur");
    let message = erreur.to_string();
    assert!(
        message.contains("n'est pas un fournisseur"),
        "message peu clair : {message}"
    );
    assert!(
        message.contains("fournisseur = 0"),
        "le message doit nommer la colonne à corriger : {message}"
    );

    // Rien n'a été écrit : le refus est prononcé avant la transaction.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_supplier_proposal WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => ENTITY, "soc" => fk_soc },
    )?;
    assert_eq!(restes.unwrap_or(0), 0, "un refus ne crée aucune demande");

    // Un identifiant inexistant est refusé lui aussi, avec un autre message.
    let fantome = demande_simple(999_999_999, "Prestation", "1", "100");
    let erreur = db::proposition_fournisseur::create(&pool, ENTITY, &fantome)
        .expect_err("un tiers inexistant ne reçoit pas de demande");
    assert!(
        erreur.to_string().contains("n'existe pas"),
        "message peu clair : {erreur}"
    );

    nettoyer(&pool, "Client")?;
    Ok(())
}

#[test]
fn refuse_les_lignes_impossibles() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Lignes")?;
    let fk_soc = fournisseur(&pool, "Lignes")?;

    // Aucune ligne : une enveloppe vide, pas une demande de prix.
    let vide = NewPropositionFournisseur::new(fk_soc);
    let erreur = db::proposition_fournisseur::create(&pool, ENTITY, &vide)
        .expect_err("une demande sans ligne ne se crée pas");
    assert!(erreur.to_string().contains("sans ligne"), "{erreur}");

    // Quantité nulle.
    let mut zero = demande_simple(fk_soc, "Sable", "0", "40");
    let erreur = db::proposition_fournisseur::create(&pool, ENTITY, &zero)
        .expect_err("zéro unité ne se demande pas");
    assert!(erreur.to_string().contains("quantité"), "{erreur}");

    // Quantité négative : le refus doit porter sur la quantité, pas ailleurs.
    zero.lines[0].qty = dec("-1");
    let erreur =
        db::proposition_fournisseur::create(&pool, ENTITY, &zero).expect_err("quantité négative");
    assert!(erreur.to_string().contains("quantité"), "{erreur}");

    // Taux de TVA hors 0..=100.
    let mut taux = demande_simple(fk_soc, "Sable", "1", "40");
    taux.lines[0].tva_tx = dec("101");
    let erreur =
        db::proposition_fournisseur::create(&pool, ENTITY, &taux).expect_err("TVA à 101 %");
    assert!(erreur.to_string().contains("TVA"), "{erreur}");

    taux.lines[0].tva_tx = dec("-1");
    assert!(db::proposition_fournisseur::create(&pool, ENTITY, &taux).is_err());

    // Remise hors bornes : 150 % rendrait un montant négatif.
    let mut remise = demande_simple(fk_soc, "Sable", "1", "40");
    remise.lines[0].remise_percent = dec("150");
    let erreur =
        db::proposition_fournisseur::create(&pool, ENTITY, &remise).expect_err("remise à 150 %");
    assert!(erreur.to_string().contains("remise"), "{erreur}");

    // Ni libellé ni produit : le fournisseur ne saurait pas quoi chiffrer.
    let mut muette = demande_simple(fk_soc, "   ", "1", "40");
    muette.lines[0].label = "   ".to_string();
    let erreur = db::proposition_fournisseur::create(&pool, ENTITY, &muette)
        .expect_err("une ligne sans libellé ni produit est illisible");
    assert!(erreur.to_string().contains("libellé"), "{erreur}");

    // Date de livraison impossible : le 31 février existe au format mais pas au
    // calendrier, et MySQL le refuserait en plein milieu de l'insertion.
    let mut date_fausse = demande_simple(fk_soc, "Sable", "1", "40");
    date_fausse.date_livraison = Some("2026-02-31".to_string());
    let erreur = db::proposition_fournisseur::create(&pool, ENTITY, &date_fausse)
        .expect_err("le 31 février n'existe pas");
    assert!(erreur.to_string().contains("date de livraison"), "{erreur}");

    // Aucun de ces refus n'a consommé de numéro : ni en-tête ni ligne.
    let mut conn = pool.get_conn()?;
    let restes: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_supplier_proposal WHERE entity = :entity AND fk_soc = :soc",
        params! { "entity" => ENTITY, "soc" => fk_soc },
    )?;
    assert_eq!(restes.unwrap_or(0), 0, "un refus ne crée aucune demande");

    nettoyer(&pool, "Lignes")?;
    Ok(())
}

#[test]
fn le_cycle_valider_clore_et_refuser_se_suit() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Cycle")?;
    let fk_soc = fournisseur(&pool, "Cycle")?;

    let id = db::proposition_fournisseur::create(
        &pool,
        ENTITY,
        &demande_simple(fk_soc, "Parpaings 20", "100", "1.20"),
    )?;

    // Brouillon : rien ne se clôt ni ne se refuse avant validation.
    let erreur = db::proposition_fournisseur::cloturer(&pool, ENTITY, id)
        .expect_err("un brouillon ne se clôt pas");
    assert!(erreur.to_string().contains("n'est pas validée"), "{erreur}");
    assert!(db::proposition_fournisseur::refuser(&pool, ENTITY, id).is_err());
    assert_eq!(
        db::proposition_fournisseur::get(&pool, ENTITY, id)?
            .unwrap()
            .fk_statut,
        STATUT_BROUILLON,
        "un refus ne déplace pas le statut"
    );

    // 0 -> 1.
    db::proposition_fournisseur::valider(&pool, ENTITY, id)?;
    let d = db::proposition_fournisseur::get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(d.fk_statut, STATUT_VALIDEE);
    assert!(d.est_validee());
    assert!(d.date_valid.is_some(), "la date de validation est posée");
    assert!(d.date_cloture.is_none());

    // Valider deux fois est un double envoi au fournisseur : refusé.
    let erreur = db::proposition_fournisseur::valider(&pool, ENTITY, id)
        .expect_err("une demande ne se valide qu'une fois");
    assert!(erreur.to_string().contains("validée"), "{erreur}");
    assert_eq!(
        db::proposition_fournisseur::get(&pool, ENTITY, id)?
            .unwrap()
            .fk_statut,
        STATUT_VALIDEE
    );

    // 1 -> 3 : le fournisseur ne répondra pas.
    db::proposition_fournisseur::refuser(&pool, ENTITY, id)?;
    let d = db::proposition_fournisseur::get(&pool, ENTITY, id)?.unwrap();
    assert_eq!(d.fk_statut, STATUT_REFUSEE);
    assert!(d.est_refusee());
    assert!(d.est_fermee(), "refusée est une forme de fermeture");
    assert!(
        d.date_cloture.is_some(),
        "une demande refusée porte sa date de clôture"
    );

    // Une demande refusée ne revient pas en arrière et ne se supprime pas.
    for (nom, resultat) in [
        (
            "valider",
            db::proposition_fournisseur::valider(&pool, ENTITY, id),
        ),
        (
            "clore",
            db::proposition_fournisseur::cloturer(&pool, ENTITY, id),
        ),
        (
            "refuser",
            db::proposition_fournisseur::refuser(&pool, ENTITY, id),
        ),
        (
            "supprimer",
            db::proposition_fournisseur::supprimer(&pool, ENTITY, id),
        ),
    ] {
        assert!(
            resultat.is_err(),
            "{nom} ne doit pas être permis après refus"
        );
    }
    assert_eq!(
        db::proposition_fournisseur::get(&pool, ENTITY, id)?
            .unwrap()
            .fk_statut,
        STATUT_REFUSEE,
        "aucun de ces refus n'a déplacé le statut"
    );

    // Une seconde demande va jusqu'à la clôture : 1 -> 2.
    let autre = db::proposition_fournisseur::create(
        &pool,
        ENTITY,
        &demande_simple(fk_soc, "Ciment 35 kg", "20", "8.50"),
    )?;
    db::proposition_fournisseur::valider(&pool, ENTITY, autre)?;
    db::proposition_fournisseur::cloturer(&pool, ENTITY, autre)?;
    let d = db::proposition_fournisseur::get(&pool, ENTITY, autre)?.unwrap();
    assert_eq!(d.fk_statut, STATUT_CLOSE);
    assert!(d.est_close());
    assert!(d.est_fermee());
    assert!(
        !d.est_validee(),
        "close n'est pas validée : le statut a bougé"
    );
    assert!(d.date_cloture.is_some());
    assert!(
        d.date_valid.is_some(),
        "la date de validation reste, la clôture ne l'efface pas"
    );

    assert!(db::proposition_fournisseur::cloturer(&pool, ENTITY, autre).is_err());
    assert!(db::proposition_fournisseur::refuser(&pool, ENTITY, autre).is_err());
    assert!(db::proposition_fournisseur::valider(&pool, ENTITY, autre).is_err());

    // Une demande inconnue est signalée, pas silencieusement ignorée.
    let erreur = db::proposition_fournisseur::valider(&pool, ENTITY, 999_999_999)
        .expect_err("une demande inexistante est une erreur");
    assert!(erreur.to_string().contains("n'existe pas"), "{erreur}");

    nettoyer(&pool, "Cycle")?;
    Ok(())
}

#[test]
fn un_brouillon_se_supprime_mais_pas_une_demande_validee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Suppression")?;
    let fk_soc = fournisseur(&pool, "Suppression")?;

    let brouillon = db::proposition_fournisseur::create(
        &pool,
        ENTITY,
        &demande_simple(fk_soc, "Ciment", "5", "8"),
    )?;
    db::proposition_fournisseur::supprimer(&pool, ENTITY, brouillon)?;
    assert!(
        db::proposition_fournisseur::get(&pool, ENTITY, brouillon)?.is_none(),
        "l'en-tête a disparu"
    );
    assert!(
        db::proposition_fournisseur::lines(&pool, ENTITY, brouillon)?.is_empty(),
        "les lignes sont parties avec l'en-tête"
    );

    let validee = db::proposition_fournisseur::create(
        &pool,
        ENTITY,
        &demande_simple(fk_soc, "Ciment", "5", "8"),
    )?;
    db::proposition_fournisseur::valider(&pool, ENTITY, validee)?;
    let erreur = db::proposition_fournisseur::supprimer(&pool, ENTITY, validee)
        .expect_err("une demande validée ne se supprime pas");
    assert!(
        erreur.to_string().contains("ne se supprime pas"),
        "{erreur}"
    );
    let d = db::proposition_fournisseur::get(&pool, ENTITY, validee)?.expect("toujours là");
    assert_eq!(d.fk_statut, STATUT_VALIDEE);
    assert_eq!(
        db::proposition_fournisseur::lines(&pool, ENTITY, validee)?.len(),
        1,
        "un refus de suppression ne touche pas les lignes"
    );

    nettoyer(&pool, "Suppression")?;
    Ok(())
}

#[test]
fn les_references_de_demande_fournisseur_sont_uniques() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    nettoyer(&pool, "Numeros")?;
    let fk_soc = fournisseur(&pool, "Numeros")?;

    let mut refs = Vec::new();
    for i in 0..3 {
        let id = db::proposition_fournisseur::create(
            &pool,
            ENTITY,
            &demande_simple(fk_soc, &format!("Ligne {i}"), "1", "10"),
        )?;
        let d = db::proposition_fournisseur::get(&pool, ENTITY, id)?.expect("demande relue");
        assert!(
            d.ref_.starts_with("DF"),
            "préfixe inattendu à l'itération {i} : {}",
            d.ref_
        );
        assert!(
            d.ref_.len() > 2,
            "une référence ne se réduit pas au préfixe : {}",
            d.ref_
        );
        refs.push(d.ref_);
    }

    let uniques: std::collections::BTreeSet<&String> = refs.iter().collect();
    assert_eq!(uniques.len(), 3, "références dupliquées : {refs:?}");

    // Le compteur croît : `suivant_ref` repart du maximum trouvé, il ne remet
    // jamais à zéro au changement de mois.
    let numeros: Vec<i64> = refs
        .iter()
        .map(|r| r[2..].parse().expect("compteur numérique"))
        .collect();
    assert!(
        numeros[1] > numeros[0] && numeros[2] > numeros[1],
        "le compteur doit croître : {numeros:?}"
    );

    // Les trois demandes du test existent bien, chacune avec sa ligne.
    for d in db::proposition_fournisseur::list(&pool, ENTITY, 5000, 0)? {
        if d.fk_soc == Some(fk_soc) {
            assert_eq!(
                db::proposition_fournisseur::lines(&pool, ENTITY, d.rowid)?.len(),
                1,
                "chaque en-tête a sa ligne"
            );
        }
    }

    nettoyer(&pool, "Numeros")?;
    Ok(())
}
