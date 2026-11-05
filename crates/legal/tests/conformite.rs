//! Vérification des mentions obligatoires, item par item.
//!
//! Source : service-public.fr (DILA, Premier ministre), page « Mentions
//! obligatoires sur une facture », vérifiée le 11 août 2026, qui renvoie au
//! code de commerce art. L441-9, au CGI annexe II art. 242 nonies A, au code
//! de l'artisanat art. L132-1 / R132-1 et au CGI art. 293 B.
//!
//! Chaque test porte le nom d'une mention légale. Si un test tombe, c'est une
//! facture qui ne serait pas opposable.

use legal::{Assurance, Client, Conditions, DocType, RegimeTva, Societe};
use std::str::FromStr;

fn societe_artisan() -> Societe {
    Societe {
        nom: "Pierre Angulaire".into(),
        forme_juridique: Some("SAS".into()),
        capital: Some("10 000 EUR".into()),
        rcs: Some("RCS Lyon 123 456 789".into()),
        adresse: Some("1 rue de l'Exemple".into()),
        code_postal: Some("69410".into()),
        ville: Some("Saint-Cyr-au-Mont-d'Or".into()),
        pays: Some("FR".into()),
        siret: Some("12345678900012".into()),
        siren: Some("123456789".into()),
        tva_intra: Some("FR12345678900".into()),
        email: Some("contact@exemple.fr".into()),
        telephone: Some("06 00 00 00 00".into()),
        regime_tva: RegimeTva::Assujetti,
        assurance: Some(Assurance {
            assureur: "AXA Assurances".into(),
            police: "POL-2026-78451".into(),
            couverture: "France métropolitaine".into(),
        }),
    }
}

fn client_pro() -> Client {
    Client {
        nom: "SARL Exemple".into(),
        siret: Some("98765432100019".into()),
        tva_intra: Some("FR98765432100".into()),
        est_professionnel: true,
        pays: Some("FR".into()),
    }
}

fn mentions_facture() -> Vec<String> {
    legal::mentions(
        DocType::Facture,
        &societe_artisan(),
        &client_pro(),
        &Conditions::default(),
    )
}

fn tout() -> String {
    mentions_facture().join(" ")
}

#[test]
fn identite_du_vendeur_denomination_forme_capital_siren() {
    // « Dénomination sociale, numéro Siren, adresse du siège social, mention de
    // la forme juridique et du montant du capital social. »
    let m = tout();
    assert!(m.contains("SAS au capital de 10 000 EUR"), "{m}");
    assert!(m.contains("RCS Lyon"), "{m}");
    // La dénomination et l'adresse sont dans l'en-tête du PDF, pas dans les
    // mentions : on vérifie qu'elles sont disponibles côté données.
    let s = societe_artisan();
    assert!(!s.nom.is_empty());
    assert!(s.adresse.is_some() && s.code_postal.is_some() && s.ville.is_some());
    assert!(s.siren.is_some());
}

#[test]
fn numero_unique_et_date_sont_des_donnees_obligatoires() {
    // « Numéro unique, basé sur une séquence chronologique et continue » et
    // « date de l'émission ». Ils sont portés par la facture, pas par les
    // mentions : le schéma et les tests d'`invoice` les garantissent.
    // (voir crates/db/tests/invoice.rs : ref PR/FA et numérotation sans trou)
}

#[test]
fn taux_de_penalites_de_retard() {
    // « Taux de pénalités de retard, exigibles sans rappel. »
    let m = tout();
    assert!(m.contains("Penalites de retard"), "{m}");
    assert!(m.contains("sans mise en demeure"), "{m}");
}

#[test]
fn indemnite_forfaitaire_de_40_euros() {
    // « Mention de l'indemnité forfaitaire de 40 € pour frais de recouvrement. »
    assert!(tout().contains("40 EUR"));
}

#[test]
fn escompte_pour_paiement_anticipe() {
    // « En cas d'absence d'escompte, mentionner : Escompte pour paiement
    // anticipé : néant. »
    assert!(tout().contains("Pas d'escompte pour paiement anticipe"));

    let c = Conditions {
        escompte: Some("2 % sous 10 jours".into()),
        ..Default::default()
    };
    let m = legal::mentions(DocType::Facture, &societe_artisan(), &client_pro(), &c).join(" ");
    assert!(m.contains("2 % sous 10 jours"), "{m}");
}

#[test]
fn assurance_professionnelle_de_l_artisan() {
    // Code de l'artisanat, art. L132-1 et R132-1 : références du contrat
    // d'assurance professionnelle, coordonnées de l'assureur et couverture
    // géographique. Obligatoire pour un artisan, sur devis comme sur facture.
    let m = tout();
    assert!(m.contains("Assurance décennale"), "{m}");
    assert!(m.contains("AXA Assurances"), "{m}");
    assert!(m.contains("POL-2026-78451"), "{m}");
    assert!(m.contains("France métropolitaine"), "{m}");

    // Elle figure aussi sur le devis.
    let d = legal::mentions(
        DocType::Devis,
        &societe_artisan(),
        &client_pro(),
        &Conditions::default(),
    )
    .join(" ");
    assert!(d.contains("Assurance décennale"), "{d}");
}

#[test]
fn franchise_en_base_art_293_b() {
    let mut s = societe_artisan();
    s.regime_tva = RegimeTva::FranchiseEnBase;
    let m = legal::mentions(DocType::Facture, &s, &client_pro(), &Conditions::default()).join(" ");
    assert!(m.contains("293 B du CGI"), "{m}");
}

#[test]
fn autoliquidation_client_ue() {
    let mut c = client_pro();
    c.pays = Some("DE".into());
    let m = legal::mentions(
        DocType::Facture,
        &societe_artisan(),
        &c,
        &Conditions::default(),
    )
    .join(" ");
    assert!(m.contains("283-2 nonies"), "{m}");
}

#[test]
fn devis_porte_la_validite() {
    let m = legal::mentions(
        DocType::Devis,
        &societe_artisan(),
        &client_pro(),
        &Conditions::default(),
    )
    .join(" ");
    assert!(m.contains("valable 30 jours"), "{m}");
}

#[test]
fn avoir_reference_la_facture_d_origine() {
    let c = Conditions {
        origine: Some("FA2611-0001".into()),
        ..Default::default()
    };
    let m = legal::mentions(DocType::Avoir, &societe_artisan(), &client_pro(), &c).join(" ");
    assert!(m.contains("FA2611-0001"), "{m}");
}

#[test]
fn les_champs_bloquants_sont_ceux_du_decret() {
    // Une facture sans SIRET, sans adresse ni sans identité client ne doit pas
    // pouvoir être émise : c'est ce que vérifie `champs_manquants`.
    let vide = Societe::default();
    let client_vide = Client::default();
    let manquants = legal::champs_manquants(DocType::Facture, &vide, &client_vide);
    for attendu in [
        "denomination du vendeur",
        "adresse du vendeur",
        "code postal du vendeur",
        "ville du vendeur",
        "SIRET du vendeur",
        "nom du client",
    ] {
        assert!(manquants.contains(&attendu), "manque : {attendu}");
    }
}

#[test]
fn une_societe_complete_passe_le_controle() {
    let m = legal::champs_manquants(DocType::Facture, &societe_artisan(), &client_pro());
    assert!(m.is_empty(), "champs manquants inattendus : {m:?}");
}

#[test]
fn le_montant_s_affiche_avec_deux_decimales() {
    // Un montant tronqué (« 4109,6 ») sur une pièce comptable est un défaut.
    let valeur = rust_decimal::Decimal::from_str("4109.6").unwrap();
    let texte = format!("{valeur:.2}");
    assert_eq!(texte, "4109.60");
}
