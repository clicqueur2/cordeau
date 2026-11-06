//! Codes-barres : calculer et vérifier une clé de contrôle.
//!
//! Ce module ne dessine **aucune barre**. Dessiner est le travail d'une
//! bibliothèque graphique ; ce qui casse en production, c'est le numéro :
//! un EAN-13 dont la clé est fausse est refusé par la caisse du client, et on
//! ne l'apprend qu'au moment du scan, quand la palette est déjà livrée.
//! Ici on produit des numéros justes et on vérifie ceux qu'on relit.
//!
//! ## Ce que dit la source Dolibarr (mesuré, pas supposé)
//!
//! Tout vient de `htdocs/core/lib/barcode.lib.php` (référence 24.0.1) :
//!
//! * `barcode_gen_ean_sum($ean)` calcule la clé, et son implémentation est
//!   **générique en longueur** : elle part du dernier chiffre, alterne les
//!   poids 3 et 1 en remontant, et rend `(10 - (somme % 10)) % 10` ;
//! * `barcode_encode_ean($ean)` n'accepte que **12 ou 13 chiffres** : Dolibarr
//!   ne sait donc pas *encoder* un EAN-8, alors que la table des types
//!   (`llx_c_barcode_type`, alimentée par `modBarcode::init()`) propose `EAN8`.
//!   La clé EAN-8 reste pourtant la même formule appliquée à 7 chiffres, et
//!   `barcode_gen_ean_sum` la donne correcte sans le savoir. On l'expose ici ;
//! * `isAValidEAN13($ean)` refait le contrôle sur 13 chiffres, exactement la
//!   convention retenue ci-dessous ;
//! * les types installés sont `EAN8`, `EAN13`, `UPC`, `ISBN`, `C39`, `C128`,
//!   `DATAMATRIX`, `QRCODE` (8 lignes, une par entité) ;
//! * **aucun algorithme `gencod` n'existe** dans cette version : ni fonction,
//!   ni type dans la table. On n'en invente pas.
//!
//! ## Où Dolibarr range un code-barres
//!
//! Il n'y a **pas** de table dédiée : le couple `(barcode, fk_barcode_type)`
//! est répété dans sept tables (`llx_product`, `llx_entrepot`, `llx_societe`,
//! `llx_user`, `llx_ticket`, `llx_product_lot`,
//! `llx_product_fournisseur_price`). Un produit porte donc un code principal,
//! plus ceux de ses lots et de ses conditions fournisseur — d'où la liste
//! rendue par [`codes_du_produit`].

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Le corps d'un EAN-13 fait toujours 12 chiffres, la clé est le 13e.
const LONGUEUR_EAN13: usize = 13;
/// Le corps d'un EAN-8 fait toujours 7 chiffres, la clé est le 8e.
const LONGUEUR_EAN8: usize = 8;

/// Contrôle qu'une chaîne est faite de exactement `attendus` chiffres ASCII.
///
/// On refuse tout le reste : espace, signe, virgule, mais aussi les chiffres
/// non latins (`٣` arabo-indien, `３` plein-chasse...). Un code-barres vit dans
/// un alphabet de dix caractères ; d'autres chiffres s'affichent comme les
/// nôtres à l'écran et produisent un numéro différent à la lecture.
fn controle_chiffres(entree: &str, attendus: usize) -> Result<(), Box<dyn Error>> {
    if entree.is_empty() {
        return Err("le code est vide".into());
    }
    if let Some((rang, c)) = entree
        .chars()
        .enumerate()
        .find(|(_, c)| !c.is_ascii_digit())
    {
        return Err(format!(
            "le code-barres ne contient que des chiffres : caractère « {c} » trouvé en \
             position {} (seuls les chiffres de 0 à 9 sont acceptés)",
            rang + 1
        )
        .into());
    }
    let n = entree.len();
    if n != attendus {
        return Err(format!(
            "longueur invalide : {n} chiffre(s) fourni(s), {attendus} attendu(s) — un EAN-13 \
             se calcule sur 12 chiffres (la clé est le 13e), un EAN-8 sur 7"
        )
        .into());
    }
    Ok(())
}

/// Clé de contrôle d'une suite de chiffres déjà validée.
///
/// `poids_fort_en_premier` dit si le **premier** chiffre (le plus à gauche)
/// porte le poids 3. C'est toute la différence entre EAN-13 et EAN-8, et elle
/// vient de la parité du nombre de chiffres de données :
///
/// * EAN-13 : **12** chiffres de données (pair), donc le dernier — celui qui
///   touche la clé — porte le poids 3, et l'alternance remonte en 1 ;
/// * EAN-8 : **7** chiffres de données (impair), donc le premier porte le
///   poids 3 pour que le dernier le porte aussi.
///
/// La règle de fond est unique : dans la somme pondérée, le chiffre collé à la
/// clé vaut toujours ×3. C'est ce qui rend le contrôle d'un seul chiffre
/// capable de détecter toutes les erreurs simples et la plupart des
/// transpositions.
fn cle_controle(chiffres: &[u8], poids_fort_en_premier: bool) -> u8 {
    let mut somme: u32 = 0;
    for (rang, chiffre) in chiffres.iter().enumerate() {
        let fort = if poids_fort_en_premier {
            rang % 2 == 0
        } else {
            rang % 2 == 1
        };
        somme += if fort {
            3 * u32::from(*chiffre)
        } else {
            u32::from(*chiffre)
        };
    }
    let reste = somme % 10;
    if reste == 0 {
        0
    } else {
        (10 - reste) as u8
    }
}

/// Rend les valeurs numériques d'une chaîne déjà contrôlée.
fn valeurs(entree: &str) -> Vec<u8> {
    entree
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| d as u8)
        .collect()
}

/// Clé de contrôle **EAN-13**, calculée sur les douze premiers chiffres.
///
/// L'algorithme n'est pas une convention maison : c'est la norme GS1. Un
/// lecteur doit pouvoir le refaire à la main, sans ouvrir de norme.
///
/// # La formule
///
/// Soit `d1 … d12` les douze chiffres, de gauche à droite. On les pondère
/// **1, 3, 1, 3, …** en partant de la gauche :
///
/// ```text
/// somme = d1×1 + d2×3 + d3×1 + d4×3 + … + d11×1 + d12×3
/// clé   = (10 − (somme mod 10)) mod 10
/// ```
///
/// Le `mod 10` extérieur évite le piège classique : quand la somme est déjà
/// multiple de 10, `10 − 0` vaut 10, qui n'est pas un chiffre. La clé est
/// alors **0**.
///
/// # Exemple chiffré, vérifiable à la main
///
/// Code `978207041311` :
///
/// ```text
/// chiffres :  9   7   8   2   0   7   0   4   1   3   1   1
/// poids    :  1   3   1   3   1   3   1   3   1   3   1   3
/// produits :  9  21   8   6   0  21   0  12   1   9   1   3
/// ```
///
/// `somme = 9+21+8+6+0+21+0+12+1+9+1+3 = 91` ; `91 mod 10 = 1` ;
/// `10 − 1 = 9` → clé **9**, soit le code complet `9782070413119`. Le préfixe
/// `978` est celui du livre : c'est d'ailleurs pourquoi Dolibarr range tout ce
/// qui commence par `978` ou `979` dans le type `ISBN` (`barcode_encode_ean`
/// force `$encoding = "ISBN"`).
///
/// # Erreurs
///
/// Toute entrée qui n'est pas exactement douze chiffres ASCII est refusée :
/// une saisie fausse doit s'arrêter ici, pas produire silencieusement un code
/// plausible.
pub fn cle_ean13(douze_chiffres: &str) -> Result<u8, Box<dyn Error>> {
    controle_chiffres(douze_chiffres, LONGUEUR_EAN13 - 1)?;
    Ok(cle_controle(&valeurs(douze_chiffres), false))
}

/// Le code à treize chiffres porte-t-il la bonne clé ?
///
/// C'est le contrôle qu'on fait à l'entrée d'un scan ou avant d'imprimer une
/// étiquette. Une longueur fausse est un refus, comme une clé fausse : dans
/// les deux cas le code n'est pas scannable.
pub fn verifie_ean13(treize_chiffres: &str) -> bool {
    if controle_chiffres(treize_chiffres, LONGUEUR_EAN13).is_err() {
        return false;
    }
    let (corps, cle) = treize_chiffres.split_at(LONGUEUR_EAN13 - 1);
    let Some(chiffre) = cle.chars().next().and_then(|c| c.to_digit(10)) else {
        return false;
    };
    matches!(cle_ean13(corps), Ok(attendue) if u32::from(attendue) == chiffre)
}

/// Clé de contrôle **EAN-8**, calculée sur les sept premiers chiffres.
///
/// Même formule que l'EAN-13, mais l'alternance démarre sur un poids **3**
/// (voir [`cle_controle`] pour la raison).
///
/// Exemple : `9638507` → `9×3 + 6×1 + 3×3 + 8×1 + 5×3 + 0×1 + 7×3 = 86` ;
/// `86 mod 10 = 6` ; `10 − 6 = 4` → `96385074`.
///
/// Dolibarr ne sait pas encoder l'EAN-8 (`barcode_encode_ean` exige 12 ou 13
/// chiffres) mais propose le type `EAN8` dans son dictionnaire : le numéro doit
/// donc être juste avant d'arriver dans l'étiquette, c'est le rôle de cette
/// fonction.
pub fn cle_ean8(huit_avant_cle: &str) -> Result<u8, Box<dyn Error>> {
    // Le nom du paramètre dit « huit », la réalité est sept : on garde le nom
    // demandé par l'appelant, la documentation tranche le malentendu.
    let corps = huit_avant_cle;
    controle_chiffres(corps, LONGUEUR_EAN8 - 1)?;
    Ok(cle_controle(&valeurs(corps), true))
}

/// Le code à huit chiffres porte-t-il la bonne clé ?
pub fn verifie_ean8(huit_chiffres: &str) -> bool {
    if controle_chiffres(huit_chiffres, LONGUEUR_EAN8).is_err() {
        return false;
    }
    let (corps, cle) = huit_chiffres.split_at(LONGUEUR_EAN8 - 1);
    let Some(chiffre) = cle.chars().next().and_then(|c| c.to_digit(10)) else {
        return false;
    };
    matches!(cle_ean8(corps), Ok(attendue) if u32::from(attendue) == chiffre)
}

/// Construit un EAN-13 à partir d'un préfixe et d'un compteur.
///
/// `prefixe` identifie le client chez GS1 (2 à 6 chiffres) ; le compteur
/// numérote les articles à l'intérieur de ce préfixe. Le compteur est complété
/// de zéros à gauche pour occuper exactement la place restante, puis la clé est
/// calculée — on ne saisit donc jamais une clé à la main.
///
/// ```text
/// generer_ean13("20", 1)   -> "200000000001" + clé
/// generer_ean13("978", 42) -> "978000000042" + clé
/// ```
///
/// # Ce que la fonction ne vérifie pas
///
/// L'appartenance réelle du préfixe à un client GS1 : elle n'est pas
/// vérifiable hors ligne, et Dolibarr ne le fait pas non plus. En revanche le
/// préfixe **2** est, dans la norme, réservé à l'usage interne du magasin
/// (balances, articles pesés) — utile quand on imprime ses propres étiquettes
/// sans être adhérent GS1.
///
/// # Erreurs
///
/// Préfixe vide, non numérique, plus court que 2 ou plus long que 6 chiffres,
/// ou compteur trop grand pour la place restante : dans ce dernier cas on
/// refuse plutôt que de tronquer, parce qu'un numéro tronqué serait un doublon
/// silencieux de l'article précédent.
pub fn generer_ean13(prefixe: &str, compteur: u64) -> Result<String, Box<dyn Error>> {
    // Le préfixe vient d'un champ de formulaire : on tolère les espaces de
    // saisie, jamais autre chose.
    let prefixe = prefixe.trim();
    if prefixe.is_empty() {
        return Err("le préfixe du code-barres est obligatoire".into());
    }
    if let Some(c) = prefixe.chars().find(|c| !c.is_ascii_digit()) {
        return Err(format!(
            "le préfixe « {prefixe} » doit être numérique : caractère « {c} » refusé"
        )
        .into());
    }
    let taille = prefixe.len();
    if !(2..=6).contains(&taille) {
        return Err(format!(
            "le préfixe « {prefixe} » fait {taille} chiffre(s) : il en faut de 2 à 6 \
             (12 chiffres de données au total, moins la place du compteur)"
        )
        .into());
    }

    let place = (LONGUEUR_EAN13 - 1) - taille;
    let compteur = compteur.to_string();
    if compteur.len() > place {
        return Err(format!(
            "le compteur {compteur} ne tient pas : avec un préfixe de {taille} chiffre(s), \
             il reste {place} position(s) sur les 12 chiffres de données, il faut donc un \
             compteur de {place} chiffres au plus"
        )
        .into());
    }

    // Complément à zéros **à gauche** : le compteur occupe la fin du numéro,
    // comme un numéro d'ordre. Écrit explicitement plutôt qu'avec une largeur
    // de formatage, pour que la lecture ne demande aucune connaissance des
    // spécificateurs.
    let zeros = "0".repeat(place - compteur.len());
    let corps = format!("{prefixe}{zeros}{compteur}");
    let cle = cle_ean13(&corps)?;
    Ok(format!("{corps}{cle}"))
}

/// Les types de code-barres du dictionnaire (`llx_c_barcode_type`), pour un
/// menu déroulant.
///
/// Rend des couples `(code, libellé)`, par exemple `("EAN13", "EAN13")` ou
/// `("C39", "Code 39")`. Le code est la valeur qu'on enregistre dans
/// `fk_barcode_type` via le dictionnaire ; le libellé est ce qu'on affiche.
///
/// Deux précautions, mesurées sur la base :
///
/// * la table est **vide tant que le module BarCode n'a pas été activé** —
///   `modBarcode::init()` est ce qui insère les huit types. On rend donc une
///   liste vide plutôt qu'une erreur : un menu vide dit la vérité (« module
///   désactivé »), une erreur ferait croire à une panne ;
/// * `modBarcode::init()` insère **une ligne par entité** (`__ENTITY__` dans
///   le SQL). On dédoublonne par code, sinon le menu afficherait huit fois le
///   même choix dès la deuxième entité.
pub fn types(pool: &Pool) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<(Option<String>, Option<String>)> =
        conn.query("SELECT code, libelle FROM llx_c_barcode_type ORDER BY rowid ASC")?;

    let mut out: Vec<(String, String)> = Vec::with_capacity(lignes.len());
    for (code, libelle) in lignes {
        let code = code.unwrap_or_default().trim().to_string();
        if code.is_empty() {
            continue;
        }
        if out.iter().any(|(c, _)| c == &code) {
            continue;
        }
        let libelle = libelle.unwrap_or_default().trim().to_string();
        let libelle = if libelle.is_empty() {
            code.clone()
        } else {
            libelle
        };
        out.push((code, libelle));
    }
    Ok(out)
}

/// Les codes-barres connus d'un produit.
///
/// Dolibarr n'a pas de table des codes-barres : la colonne `barcode` est
/// recopiée dans plusieurs tables. Pour un produit, trois sources existent :
///
/// 1. `llx_product.barcode` — le code principal, celui qu'on imprime ;
/// 2. `llx_product_lot.barcode` — un code par lot, quand le fournisseur
///    étiquette ses lots ;
/// 3. `llx_product_fournisseur_price.barcode` — le code du fournisseur pour
///    cette référence, qui n'est pas le nôtre.
///
/// On rend le code principal en tête, puis les autres **sans doublon** : un
/// même numéro peut apparaître à la fois sur le produit et sur un lot, et
/// l'afficher deux fois dans une fiche fait douter de la donnée.
///
/// Une liste vide est un résultat normal : c'est l'état de la base livrée, où
/// aucun produit ne porte de code-barres.
pub fn codes_du_produit(
    pool: &Pool,
    entity: i64,
    fk_product: i64,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let mut out: Vec<String> = Vec::new();

    let principal: Option<Option<String>> = conn.exec_first(
        "SELECT barcode FROM llx_product WHERE entity = :entity AND rowid = :produit",
        params! { "entity" => entity, "produit" => fk_product },
    )?;
    // `None` = produit inexistant ; `Some(None)` = produit sans code.
    let Some(principal) = principal else {
        return Err(format!("produit {fk_product} introuvable dans l'entité {entity}").into());
    };
    pousser(&mut out, principal);

    let lots: Vec<Option<String>> = conn.exec(
        // `entity` est nullable sur `llx_product_lot` : une ligne historique
        // peut ne pas la porter. On la garde plutôt que de perdre un code.
        "SELECT barcode FROM llx_product_lot \
         WHERE fk_product = :produit AND (entity = :entity OR entity IS NULL) \
         ORDER BY rowid ASC",
        params! { "entity" => entity, "produit" => fk_product },
    )?;
    for code in lots {
        pousser(&mut out, code);
    }

    let fournisseurs: Vec<Option<String>> = conn.exec(
        "SELECT barcode FROM llx_product_fournisseur_price \
         WHERE fk_product = :produit AND entity = :entity AND barcode IS NOT NULL \
         ORDER BY rowid ASC",
        params! { "entity" => entity, "produit" => fk_product },
    )?;
    for code in fournisseurs {
        pousser(&mut out, code);
    }

    Ok(out)
}

/// Ajoute un code s'il est renseigné et pas déjà présent.
fn pousser(codes: &mut Vec<String>, code: Option<String>) {
    let Some(code) = code else { return };
    let code = code.trim().to_string();
    if code.is_empty() || codes.contains(&code) {
        return;
    }
    codes.push(code);
}
