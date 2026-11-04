//! TVA non deductible (`tva_npr`) : saisie d'un taux suffixe d'une etoile.
//!
//! Dolibarr n'a pas de colonne « taux non deductible » : le drapeau vit dans
//! `llx_*det.info_bits`, bit 0, et le taux stocke (`tva_tx`) reste le taux nu.
//! L'etoile est une **convention de saisie**, pas une valeur en base.
//!
//! # Ce que le `*` change reellement dans `calcul_price_total`
//!
//! Le `*` ne modifie **que le taux de TVA principal**. Les taxes locales
//! (`localtax1`, `localtax2`) ne sont pas touchees.
//!
//! Chaine de preuve, dans `htdocs/core/lib/price.lib.php` (Dolibarr 24.0.1) :
//!
//! 1. La saisie est decoupee **avant** l'appel : `fourn/facturefourn.php:1911`
//!    fait `$tva_npr = (preg_match('/\*/', $tva_tx) ? 1 : 0);` puis ligne 1912
//!    `$tva_tx = str_replace('*', '', $tva_tx);`. Le `*` devient donc le bit 0
//!    de `$info_bits`, transmis en 9e argument a `calcul_price_total` par
//!    `fourn/class/fournisseur.facture.class.php:2254`, et `$txtva` y arrive
//!    **sans** etoile, avec le taux pur (cf. le docblock de la fonction,
//!    ligne 44 : « this is VAT rate only without text code »).
//! 2. Dans `calcul_price_total`, le taux n'est neutralise que par ce test :
//!    `(($info_bits & 1) ? 0 : $txtva)` — **ligne 303** (`$result[8]`,
//!    total TTC sans remise), **ligne 308** (`$result[2]`, total TTC) et
//!    **ligne 313** (`$result[5]`, PU TTC) pour la branche `price_base_type
//!    != 'TTC'` ; **lignes 319, 324 et 329** pour la branche TTC.
//! 3. Les taxes locales, elles, sont ajoutees **sans condition** dans ces
//!    memes expressions : `+ $localtaxes[0]` (ligne 303), `+ $localtaxes[1]`
//!    (ligne 308), `+ $localtaxes[2]` (ligne 313). Leurs blocs de calcul
//!    (lignes 262-297 pour les taxes assises sur le TTC, lignes 353-388 pour
//!    celles assises sur le HT) ne lisent **jamais** `$info_bits`. Seule la
//!    TVA principale est donc mise a zero.
//! 4. Le drapeau n'influe pas non plus sur les taux de taxes locales en amont
//!    pris par la branche « ligne libre » : `facturefourn.php:1925-1926`
//!    appelle `get_localtax($tva_tx, 1|2, ...)` **sans** `$tva_npr`, juste
//!    apres avoir justement extrait l'etoile.
//!
//! # La subtilite a ne pas rater : le montant de TVA existe quand meme
//!
//! `$result[1]` (`total_vat`, ligne 310) est calcule a partir de `$result2bis`
//! (**ligne 309**), qui emploie le **taux plein** : la TVA est donc toujours
//! evaluee et stockee. C'est le **TTC** qui l'exclut (ligne 308, ligne 324 en
//! TTC). Consequence a respecter partout : sur une ligne NPR,
//! `total_ttc != total_ht + total_tva`. Le montant de TVA existe (il doit etre
//! ecrit en comptabilite, mais en charge et non en 44566).

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;

/// Un taux de TVA tel que saisi par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TauxSaisi {
    /// Le taux nu, etoile retiree : `20` pour `"20*"`.
    pub taux: Decimal,
    /// Vrai si la saisie portait une etoile (TVA non deductible).
    pub non_deductible: bool,
}

/// Pourquoi une saisie de taux a ete refusee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erreur {
    /// Rien a lire : `""`, `"   "`, ou une etoile seule `"*"`.
    Vide,
    /// Le texte n'est pas un nombre : `"abc"`, `"20 %"`, `"20,5,5"`.
    TauxIllisible(String),
    /// Un taux de TVA ne peut pas etre negatif : `"-5"`, `"-5*"`.
    Negatif(String),
}

impl fmt::Display for Erreur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erreur::Vide => write!(f, "aucun taux de TVA saisi"),
            Erreur::TauxIllisible(s) => write!(f, "taux de TVA illisible : « {s} »"),
            Erreur::Negatif(s) => write!(f, "taux de TVA negatif : « {s} »"),
        }
    }
}

impl Error for Erreur {}

/// Lit une saisie de taux, avec ou sans etoile de TVA non deductible.
///
/// Accepte `"20"`, `"20*"`, `"5,5*"`, `" 20 * "` (la virgule decimale et les
/// espaces sont toleres, plusieurs etoiles equivalent a une). Le `*` est retire
/// du taux, comme le fait `facturefourn.php:1912`.
pub fn analyser(saisie: &str) -> Result<TauxSaisi, Erreur> {
    let brut = saisie.trim();
    if brut.is_empty() {
        return Err(Erreur::Vide);
    }

    // Un `*` quelque part signifie « non deductible » ; on le retire partout.
    let non_deductible = brut.contains('*');
    let sans_etoile = brut.replace('*', "");
    let nombre = sans_etoile.trim();
    if nombre.is_empty() {
        // Cas de l'etoile seule : `"*"` ou `" * "`.
        return Err(Erreur::Vide);
    }

    let taux = Decimal::from_str(&nombre.replace(',', "."))
        .map_err(|_| Erreur::TauxIllisible(brut.to_string()))?;
    if taux.is_sign_negative() {
        return Err(Erreur::Negatif(brut.to_string()));
    }

    Ok(TauxSaisi {
        taux,
        non_deductible,
    })
}

/// Somme des montants de TVA **deductibles**.
///
/// Entree : `(taux, non_deductible, montant_tva)`. Une ligne non deductible ne
/// compte pas : sa TVA est une charge, pas un credit de TVA a reporter en
/// 44566. Le taux n'est pas utilise ici (il sert a l'appelant pour le
/// regroupement par taux) mais reste dans la signature pour que l'appelant ne
/// detache pas les colonnes.
#[must_use]
pub fn tva_deductible(lignes: &[(Decimal, bool, Decimal)]) -> Decimal {
    lignes
        .iter()
        .filter(|(_, non_deductible, _)| !*non_deductible)
        .map(|(_, _, montant)| *montant)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal de test")
    }

    #[test]
    fn vingt_sans_etoile_est_deductible() {
        let t = analyser("20").expect("20 doit etre lu");
        assert_eq!(t.taux, d("20"));
        assert!(!t.non_deductible);
    }

    #[test]
    fn vingt_etoile_est_non_deductible() {
        let t = analyser("20*").expect("20* doit etre lu");
        assert_eq!(t.taux, d("20"));
        assert!(t.non_deductible);
    }

    #[test]
    fn cinq_cinq_etoile_avec_virgule() {
        let t = analyser("5,5*").expect("5,5* doit etre lu");
        assert_eq!(t.taux, d("5.5"));
        assert!(t.non_deductible);
    }

    #[test]
    fn espaces_autour_de_la_saisie() {
        let t = analyser(" 20 * ").expect("«  20 *  » doit etre lu");
        assert_eq!(t.taux, d("20"));
        assert!(t.non_deductible);
    }

    #[test]
    fn refus_quand_vide() {
        assert_eq!(analyser(""), Err(Erreur::Vide));
        assert_eq!(analyser("   "), Err(Erreur::Vide));
    }

    #[test]
    fn refus_quand_etoile_seule() {
        assert_eq!(analyser("*"), Err(Erreur::Vide));
        assert_eq!(analyser(" * "), Err(Erreur::Vide));
    }

    #[test]
    fn refus_quand_illisible() {
        assert_eq!(
            analyser("abc"),
            Err(Erreur::TauxIllisible("abc".to_string()))
        );
    }

    #[test]
    fn refus_quand_negatif() {
        assert_eq!(analyser("-5"), Err(Erreur::Negatif("-5".to_string())));
        assert_eq!(analyser("-5*"), Err(Erreur::Negatif("-5*".to_string())));
    }

    #[test]
    fn tva_deductible_exclut_la_ligne_non_deductible() {
        // 200,00 de TVA a 20 % deductible
        // 100,00 de TVA a 20 % NON deductible (charge, pas de credit de TVA)
        //  55,00 de TVA a 5,5 % deductible
        let lignes = [
            (d("20"), false, d("200.00")),
            (d("20"), true, d("100.00")),
            (d("5.5"), false, d("55.00")),
        ];
        assert_eq!(tva_deductible(&lignes), d("255.00"));
    }

    #[test]
    fn tva_deductible_sans_ligne() {
        assert_eq!(tva_deductible(&[]), Decimal::ZERO);
    }

    #[test]
    fn tva_deductible_tout_non_deductible() {
        let lignes = [(d("20"), true, d("120.00")), (d("10"), true, d("30.00"))];
        assert_eq!(tva_deductible(&lignes), Decimal::ZERO);
    }
}
