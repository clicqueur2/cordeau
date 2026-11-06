//! Codes-barres : ce qui se vérifie sans base (le calcul de la clé) et ce qui
//! se lit en base (les types du dictionnaire, les codes d'un produit).
//!
//! Le module `code_barre` n'est pas encore déclaré dans `crates/db/src/lib.rs`
//! et je n'ai pas le droit de modifier un fichier existant : on inclut donc la
//! source par son chemin. Ça ne change rien aux tests — ils appellent le vrai
//! fichier, pas une copie. Le jour où `pub mod code_barre;` sera ajouté, la
//! ligne `#[path]` ci-dessous pourra disparaître sans toucher aux tests.

use db::code_barre;
use std::error::Error;

use mysql::Pool;

const ENTITY: i64 = 1;

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignoré");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Pur : la clé EAN-13
// ---------------------------------------------------------------------------

/// La clé d'un EAN-13 connu, recalculée sans base.
///
/// `978207041311` est le corps d'un vrai code livre : la somme pondérée vaut
/// 91, donc la clé est 9. `590123412345` est l'exemple canonique de la norme
/// GS1 (clé 7). Deux codes de familles différentes, pour ne pas tester une
/// seule série de chiffres.
#[test]
fn la_cle_dun_ean13_connu_est_juste() {
    assert_eq!(code_barre::cle_ean13("978207041311").unwrap(), 9);
    assert_eq!(code_barre::cle_ean13("590123412345").unwrap(), 7);
}

/// Quand la somme est déjà multiple de 10, la clé vaut 0 et non 10.
///
/// C'est l'erreur classique du calcul : `10 − (somme % 10)` donne 10, qui
/// n'est pas un chiffre. La formule doit garder un second modulo.
#[test]
fn une_somme_multiple_de_dix_donne_la_cle_zero() {
    assert_eq!(code_barre::cle_ean13("000000000000").unwrap(), 0);
    assert!(code_barre::verifie_ean13("0000000000000"));
}

#[test]
fn verifie_ean13_accepte_un_code_juste() {
    assert!(code_barre::verifie_ean13("9782070413119"));
    assert!(code_barre::verifie_ean13("5901234123457"));
}

/// Deux fautes différentes : la clé fausse, et un chiffre du corps qui a
/// bougé. La seconde est la plus dangereuse — la clé ne correspond plus, mais
/// rien dans le numéro ne dit *où* est l'erreur.
#[test]
fn verifie_ean13_refuse_une_cle_fausse() {
    assert!(
        !code_barre::verifie_ean13("9782070413118"),
        "clé 8 au lieu de 9"
    );
    assert!(
        !code_barre::verifie_ean13("9782071413119"),
        "un 1 déplacé dans le corps : la clé 9 ne vaut plus"
    );
    // Une transposition de deux chiffres voisins doit aussi être vue : le
    // poids 1/3 alterné est là pour ça.
    assert!(!code_barre::verifie_ean13("9782070143119"));
}

#[test]
fn refuse_une_longueur_invalide() {
    // Le calcul de clé exige exactement douze chiffres.
    assert!(code_barre::cle_ean13("97820704131").is_err(), "11 chiffres");
    assert!(
        code_barre::cle_ean13("9782070413119").is_err(),
        "13 chiffres"
    );
    assert!(code_barre::cle_ean13("").is_err(), "vide");

    // La vérification exige treize chiffres : ni douze, ni quatorze.
    assert!(!code_barre::verifie_ean13("978207041311"));
    assert!(!code_barre::verifie_ean13("97820704131199"));
    assert!(!code_barre::verifie_ean13(""));
}

/// Un espace ou une lettre ne doivent jamais être rognés en silence : un code
/// « presque juste » est un code faux, et le refus doit dire pourquoi.
#[test]
fn refuse_des_caracteres_non_numeriques() {
    assert!(code_barre::cle_ean13("97820704131A").is_err());
    assert!(
        code_barre::cle_ean13("978207041311 ").is_err(),
        "espace final"
    );
    assert!(
        code_barre::cle_ean13("97 8207041311").is_err(),
        "espace interne"
    );
    assert!(code_barre::cle_ean13("97820704131-").is_err());
    // Chiffres arabo-indiens : ils s'affichent comme les nôtres mais ne sont
    // pas dans l'alphabet du code-barres.
    assert!(code_barre::cle_ean13("٩٧٨٢٠٧٠٤١٣١١").is_err());

    assert!(!code_barre::verifie_ean13("97820704131A9"));
    assert!(!code_barre::verifie_ean13("978207041311 "));
}

// ---------------------------------------------------------------------------
// Pur : la clé EAN-8
// ---------------------------------------------------------------------------

/// L'EAN-8 inverse l'alternance : 7 chiffres de données, donc le premier porte
/// le poids 3. `9638507` est l'exemple canonique (clé 4).
#[test]
fn la_cle_ean8_est_juste() {
    assert_eq!(code_barre::cle_ean8("9638507").unwrap(), 4);
    // Somme = 60, multiple de 10 : clé 0.
    assert_eq!(code_barre::cle_ean8("1234567").unwrap(), 0);
}

#[test]
fn verifie_ean8_juste_et_faux() {
    assert!(code_barre::verifie_ean8("96385074"));
    assert!(code_barre::verifie_ean8("12345670"));
    assert!(!code_barre::verifie_ean8("96385075"));
    // Un EAN-13 n'est pas un EAN-8 : la longueur suffit à le refuser.
    assert!(!code_barre::verifie_ean8("9782070413119"));
    assert!(code_barre::cle_ean8("963850").is_err(), "6 chiffres");
    assert!(code_barre::cle_ean8("96385074").is_err(), "8 chiffres");
}

// ---------------------------------------------------------------------------
// Pur : génération
// ---------------------------------------------------------------------------

/// Le compteur est complété de zéros à gauche, puis la clé est calculée : le
/// numéro produit doit passer sa propre vérification.
#[test]
fn generer_ean13_produit_un_code_valide() {
    let code = code_barre::generer_ean13("20", 1).expect("génération");
    assert_eq!(code.len(), 13, "un EAN-13 fait 13 chiffres : {code}");
    assert!(code.starts_with("20"), "préfixe conservé : {code}");
    assert_eq!(&code[..12], "200000000001", "compteur complété de zéros");
    assert!(code_barre::verifie_ean13(&code), "code non valide : {code}");

    let livre = code_barre::generer_ean13("978", 42).expect("génération");
    assert_eq!(&livre[..12], "978000000042");
    assert!(code_barre::verifie_ean13(&livre));

    // Préfixe de 6 chiffres : le compteur n'a plus que 6 places.
    let court = code_barre::generer_ean13("123456", 999_999).expect("génération");
    assert_eq!(court, {
        let corps = "123456999999".to_string();
        let cle = code_barre::cle_ean13(&corps).expect("clé");
        format!("{corps}{cle}")
    });
    assert!(code_barre::verifie_ean13(&court));
}

/// Deux compteurs successifs ne doivent pas donner le même numéro : c'est
/// toute la raison d'être du compteur. Et les deux doivent rester valides.
#[test]
fn deux_compteurs_successifs_donnent_deux_codes_differents() {
    let premier = code_barre::generer_ean13("20", 1).expect("premier");
    let second = code_barre::generer_ean13("20", 2).expect("second");
    assert_ne!(premier, second, "{premier} et {second} identiques");
    assert!(code_barre::verifie_ean13(&premier));
    assert!(code_barre::verifie_ean13(&second));
    // L'écart est bien dans le compteur, pas dans la clé.
    assert_ne!(&premier[..12], &second[..12]);
}

#[test]
fn generer_ean13_refuse_un_prefixe_invalide() {
    // Non numérique.
    assert!(code_barre::generer_ean13("2A", 1).is_err());
    assert!(code_barre::generer_ean13("  ", 1).is_err());
    // Trop court (1 chiffre) ou trop long (7).
    assert!(code_barre::generer_ean13("1", 1).is_err());
    assert!(code_barre::generer_ean13("1234567", 1).is_err());
    // Compteur qui déborde la place restante : on refuse au lieu de tronquer,
    // sinon deux articles partageraient le même numéro.
    assert!(code_barre::generer_ean13("20", u64::MAX).is_err());
    assert!(code_barre::generer_ean13("123456", 1_000_000).is_err());
    // Les espaces de saisie sont tolérés, pas les autres caractères.
    assert!(code_barre::generer_ean13(" 20 ", 7).is_ok());
}

// ---------------------------------------------------------------------------
// Avec la base
// ---------------------------------------------------------------------------

/// Les types viennent de `llx_c_barcode_type`, alimentée à l'activation du
/// module BarCode (`modBarcode::init()`). Sur une base où le module est
/// désactivé, la table est vide : la liste vide est alors la bonne réponse, et
/// le test ne doit pas échouer pour autant.
#[test]
fn les_types_de_code_barres_se_lisent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let types = code_barre::types(&pool)?;

    if types.is_empty() {
        eprintln!(
            "llx_c_barcode_type est vide : le module BarCode n'a pas été activé sur cette base"
        );
        return Ok(());
    }

    assert!(
        types
            .iter()
            .all(|(code, libelle)| !code.is_empty() && !libelle.is_empty()),
        "aucun type sans code ni libellé : {types:?}"
    );
    // Le dédoublonnage par code doit tenir même si plusieurs entités ont
    // inséré les mêmes huit lignes.
    let mut codes: Vec<&str> = types.iter().map(|(c, _)| c.as_str()).collect();
    codes.sort_unstable();
    let avant = codes.len();
    codes.dedup();
    assert_eq!(
        avant,
        codes.len(),
        "un même code apparaît deux fois : {types:?}"
    );

    // Si EAN13 est là, c'est bien le code attendu par `fk_barcode_type`.
    for (code, libelle) in &types {
        if code.as_str() == "EAN13" {
            assert_eq!(libelle.as_str(), "EAN13");
        }
    }
    Ok(())
}

/// Un code-barres de produit est stocké dans `llx_product.barcode` — pas dans
/// une table dédiée. On en crée un, on le relit, puis on efface : la base de
/// travail ne doit pas accumuler les produits de test.
#[test]
fn les_codes_du_produit_se_lisent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    const REF: &str = "ZZCODEBAR1";
    if let Some(ancien) = db::product::by_ref(&pool, ENTITY, REF)? {
        db::product::delete(&pool, ENTITY, ancien.rowid)?;
    }

    let code = code_barre::generer_ean13("20", 1)?;
    let mut nouveau = db::product::NewProduct::new(REF, "Article étiqueté");
    nouveau.barcode = Some(code.clone());
    let id = db::product::create(&pool, ENTITY, &nouveau)?;

    let codes = code_barre::codes_du_produit(&pool, ENTITY, id)?;
    assert_eq!(
        codes,
        vec![code.clone()],
        "le code principal doit sortir seul"
    );
    assert!(code_barre::verifie_ean13(&codes[0]), "code relu invalide");

    // Un produit sans code rend une liste vide, pas une erreur : c'est l'état
    // normal de la base livrée (aucun produit n'y porte de code-barres).
    // `NewProduct::new` ne pose aucun code, c'est exactement le cas null.
    let sans = db::product::NewProduct::new("ZZCODEBAR2", "Article sans code");
    let sans_id = db::product::create(&pool, ENTITY, &sans)?;
    assert!(code_barre::codes_du_produit(&pool, ENTITY, sans_id)?.is_empty());

    // Un identifiant inconnu est une erreur : une fiche vide ferait croire à
    // un produit sans code.
    assert!(code_barre::codes_du_produit(&pool, ENTITY, 999_999_999).is_err());

    db::product::delete(&pool, ENTITY, id)?;
    db::product::delete(&pool, ENTITY, sans_id)?;
    Ok(())
}

/// Les produits déjà présents dans la base se lisent sans erreur, qu'ils
/// portent un code ou non.
#[test]
fn les_produits_existants_se_lisent_sans_erreur() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    for produit in db::product::list(&pool, ENTITY, 10, 0)? {
        let codes = code_barre::codes_du_produit(&pool, ENTITY, produit.rowid)?;
        for code in &codes {
            assert!(
                !code.trim().is_empty(),
                "un code vide ne doit pas sortir pour {}",
                produit.ref_
            );
        }
    }
    Ok(())
}
