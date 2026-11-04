use einvoicing::{Statut, StatutVie};

// ---------------------------------------------------------------------------
// Les 14 statuts du cycle de vie
// ---------------------------------------------------------------------------

/// Les quatorze statuts existent, et quatre seulement sont obligatoires.
#[test]
fn quatre_statuts_sont_obligatoires() {
    let obligatoires: Vec<u16> = StatutVie::TOUS
        .iter()
        .filter(|s| s.obligatoire())
        .map(|s| s.code())
        .collect();
    assert_eq!(obligatoires, vec![200, 210, 212, 213]);
    assert_eq!(StatutVie::TOUS.len(), 14);
}

/// Code et statut se relisent dans les deux sens, sans trou ni doublon.
#[test]
fn les_codes_200_a_213_se_relisent() {
    for code in 200..=213u16 {
        let statut = StatutVie::depuis_code(code).unwrap_or_else(|| panic!("code {code} absent"));
        assert_eq!(statut.code(), code);
        assert!(!statut.libelle().is_empty());
    }
    assert_eq!(StatutVie::depuis_code(199), None);
    assert_eq!(StatutVie::depuis_code(214), None);
}

/// Les statuts que seul un tiers peut poser : ils viennent de la plateforme,
/// jamais du logiciel.
#[test]
fn les_statuts_sortants_sont_ceux_du_logiciel() {
    let entrants: Vec<u16> = StatutVie::TOUS
        .iter()
        .filter(|s| s.entrant())
        .map(|s| s.code())
        .collect();
    assert_eq!(entrants, vec![200, 201, 202, 203, 213]);

    // Les quatre que le logiciel produit lui-meme ne sont pas des receptions.
    for code in [205u16, 210, 211, 212] {
        let s = StatutVie::depuis_code(code).unwrap();
        assert!(!s.entrant(), "le statut {code} doit pouvoir etre emis");
    }
}

/// Seuls le refus et le rejet ferment l'echange.
#[test]
fn seuls_le_refus_et_le_rejet_sont_terminaux() {
    let terminaux: Vec<u16> = StatutVie::TOUS
        .iter()
        .filter(|s| s.terminal())
        .map(|s| s.code())
        .collect();
    assert_eq!(terminaux, vec![210, 213]);
}

/// Le JSON de la plateforme se lit : « 213 » donne un rejet type.
#[test]
fn un_statut_json_se_relit_en_statut_de_la_norme() {
    let s: Statut = serde_json::from_str(r#"{"status":"213","reason":"DOUBLON"}"#).unwrap();
    assert_eq!(s.vie(), Some(StatutVie::Rejetee));
    assert!(s.vie().unwrap().terminal());
    assert_eq!(s.motif.as_deref(), Some("DOUBLON"));

    // Un libelle n'est pas un code : on ne devine pas.
    let libelle: Statut = serde_json::from_str(r#"{"status":"REJETEE"}"#).unwrap();
    assert_eq!(libelle.vie(), None);
}
