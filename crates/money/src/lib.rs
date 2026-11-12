//! Arithmetique monetaire reproduisant Dolibarr `price.lib.php::calcul_price_total()`.
//!
//! Regles reproduites :
//! - arrondi PHP `round()` = demi hors de zero (et non bancaire) ;
//! - totaux arrondis a 2 decimales via l'equivalent de `price2num($x, 'MT')` ;
//! - prix unitaires conserves a 6 decimales (`price2num($x, 'MU')`) ;
//! - aucun arrondi intermediaire : HT et TTC sont arrondis separement a partir
//!   du montant brut `pu * qty * (1 - remise/100)`, puis la TVA est la
//!   difference des deux arrondis (`total_vat = total_ttc - total_ht`). C'est ce
//!   qui garantit `ht + tva == ttc` et c'est pourquoi on ne calcule jamais
//!   `tva = ht * taux` : sur 1,5 x 19,99 a 10 %, Dolibarr donne 29,99 + 2,99 =
//!   32,98 et non 29,99 + 3,00 ;
//! - base TTC : le HT est extrait du *total* (`total / (1 + taux)`), jamais du
//!   prix unitaire arrondi, sinon une quantite elevee ferait deriver le total
//!   (0,5e-6 x 1 000 000 = 0,50 EUR d'ecart).
//!
//! Particularite Dolibarr assumee : pour une ligne en TVA non percue
//! (`info_bits & 1`), le TTC ignore la TVA mais `total_vat` est quand meme la
//! TVA "comme si elle etait normale" (le code amont utilise `$result2bis` /
//! `$result0bis`, sans `$info_bits`). La ligne n'est donc pas coherente
//! (`ht + tva != ttc`) des que `taux != 0` ; en pratique la TVA est a 0 sur ces
//! lignes et l'ecart disparait. On reproduit le comportement, il est verifie
//! par `tests/fixtures/oracle.json`.

use rust_decimal::{Decimal, RoundingStrategy};
use std::str::FromStr;

pub const SCALE_UNIT: u32 = 6;
pub const SCALE_MONEY: u32 = 2;

/// `price2num(..., 'MT')` : 2 decimales, demi hors de zero.
pub fn round_money(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(SCALE_MONEY, RoundingStrategy::MidpointAwayFromZero)
}

/// `price2num(..., 'MU')` : 6 decimales, demi hors de zero.
pub fn round_unit_price(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(SCALE_UNIT, RoundingStrategy::MidpointAwayFromZero)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceBase {
    Ht,
    Ttc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VatMode {
    /// TVA collectee normalement.
    Normal,
    /// `info_bits & 1` : TVA non percue (autoliquidation, franchise).
    NotCollected,
}

#[derive(Debug, Clone)]
pub struct LineInput {
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub discount_percent: Decimal,
    pub vat_rate: Decimal,
    pub base: PriceBase,
    pub vat_mode: VatMode,
}

impl LineInput {
    pub fn new(qty: &str, unit_price: &str, discount_percent: &str, vat_rate: &str) -> Self {
        Self {
            qty: dec(qty),
            unit_price: dec(unit_price),
            discount_percent: dec(discount_percent),
            vat_rate: dec(vat_rate),
            base: PriceBase::Ht,
            vat_mode: VatMode::Normal,
        }
    }

    pub fn ttc(mut self) -> Self {
        self.base = PriceBase::Ttc;
        self
    }

    pub fn not_collected(mut self) -> Self {
        self.vat_mode = VatMode::NotCollected;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineTotals {
    pub unit_price_ht: Decimal,
    pub total_ht_before_discount: Decimal,
    pub discount_amount: Decimal,
    pub total_ht: Decimal,
    pub total_vat: Decimal,
    pub total_ttc: Decimal,
}

/// Un decimal **connu d'avance** : constantes du code, chaines deja validees.
///
/// Panique sur une entree illisible — c'est voulu pour les constantes, ou
/// l'echec ne peut venir que d'une faute de frappe dans le source. **Ne jamais
/// l'appeler sur une valeur venue d'un client** : utiliser [`tenter`].
pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal invalide")
}

/// Magnitude maximale acceptee pour un montant, une quantite ou un taux.
///
/// `10^12` : mille milliards. Le produit de deux valeurs bornees (`10^24`) reste
/// tres loin du maximum de `Decimal` (`7,9e28`), donc aucune multiplication
/// interne ne peut plus deborder.
// 10^12 en mots de 32 bits : 232 * 2^32 + 3 567 587 328. Le test
// `max_montant_vaut_un_billion` verifie l'egalite avec « 1000000000000 ».
pub const MAX_MONTANT: Decimal = Decimal::from_parts(3_567_587_328, 232, 0, false, 0);

/// Un decimal venu de l'exterieur : rend une erreur lisible au lieu de paniquer.
///
/// Accepte la virgule decimale (`12,50`) et les espaces de milliers, parce que
/// c'est ce qu'on trouve dans un CSV ou un formulaire francais.
pub fn tenter(s: &str) -> Result<Decimal, String> {
    let brut = s.trim();
    if brut.is_empty() {
        return Err("valeur vide".to_string());
    }
    let nettoye: String = brut
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{00a0}' && *c != '\u{20ac}')
        .collect();
    let valeur = Decimal::from_str(&nettoye.replace(',', "."))
        .map_err(|_| format!("« {brut} » n'est pas un nombre"))?;
    // Mille milliards d'euros, dans un sens ou dans l'autre. Au-dela, ce n'est
    // plus un montant : c'est une faute de frappe. La borne existe parce que
    // `rust_decimal` **panique** quand une multiplication depasse 7,9e28 — le
    // fil de la requete mourait et le client recevait un 500 muet.
    if valeur.abs() > MAX_MONTANT {
        return Err(format!(
            "« {brut} » depasse la limite de {} (mille milliards)",
            MAX_MONTANT
        ));
    }
    Ok(valeur)
}

pub fn line_totals(i: &LineInput) -> LineTotals {
    // Derniere barriere : une valeur relue en base ou importee ne passe pas par
    // `tenter`. `rust_decimal` panique quand `*` ou `+` depassent 7,9e28, ce
    // qui tuait le fil de la requete. On borne donc a l'entree du calcul ; les
    // portes (API, MCP, formulaires) refusent deja ces valeurs avec un message,
    // ceci ne se declenche que sur une donnee deja fausse en base.
    let borner = |v: Decimal| {
        if v > MAX_MONTANT {
            MAX_MONTANT
        } else if v < -MAX_MONTANT {
            -MAX_MONTANT
        } else {
            v
        }
    };
    let i = &LineInput {
        qty: borner(i.qty),
        unit_price: borner(i.unit_price),
        discount_percent: borner(i.discount_percent),
        vat_rate: borner(i.vat_rate),
        ..i.clone()
    };
    let hundred = dec("100");

    // `$tot_sans_remise = $pu * $qty * ($progress / 100)` puis
    // `$tot_avec_remise_ligne = $tot_sans_remise * (1 - $remise / 100)`.
    // Aucun arrondi intermediaire : les montants sont bruts jusqu'a price2num().
    let gross = i.unit_price * i.qty;
    let net = gross * (Decimal::ONE - i.discount_percent / hundred);

    // Taux effectivement applique : `$info_bits & 1` (TVA non percue) force 0.
    let effective_vat = match i.vat_mode {
        VatMode::NotCollected => Decimal::ZERO,
        VatMode::Normal => i.vat_rate,
    };
    let one_plus_effective_vat = Decimal::ONE + effective_vat / hundred;
    // Taux "comme si la TVA etait normale" : sert au calcul du montant de TVA,
    // que Dolibarr calcule toujours avec le taux plein (voir plus bas).
    let one_plus_normal_vat = Decimal::ONE + i.vat_rate / hundred;

    // Reproduit exactement `calcul_price_total()` : le HT et le TTC sont arrondis
    // separement a partir du montant brut, puis la TVA est la difference des deux
    // arrondis (`Total VAT = TTC - HT`), ce qui garantit ttc = ht + tva.
    let (total_ht, total_ttc, total_ht_before_discount, unit_price_ht) = match i.base {
        PriceBase::Ht => {
            // price_base_type != 'TTC'
            (
                round_money(net),
                round_money(net * one_plus_effective_vat),
                round_money(gross),
                round_unit_price(i.unit_price),
            )
        }
        PriceBase::Ttc => {
            // price_base_type == 'TTC' : le prix saisi est TTC, on en extrait le HT
            // a partir du total (et non du prix unitaire arrondi).
            (
                round_money(net / one_plus_effective_vat),
                round_money(net),
                round_money(gross / one_plus_effective_vat),
                round_unit_price(i.unit_price / one_plus_effective_vat),
            )
        }
    };

    // `$result2bis` / `$result0bis` : montant TTC ou HT recalcule avec le taux plein,
    // meme quand la ligne est en TVA non percue. Dolibarr ne tient pas compte de
    // `$info_bits` ici ; on reproduit son resultat, y compris pour les lignes NPR.
    let total_vat = match i.base {
        PriceBase::Ht => round_money(round_money(net * one_plus_normal_vat) - total_ht),
        PriceBase::Ttc => round_money(total_ttc - round_money(net / one_plus_normal_vat)),
    };

    LineTotals {
        unit_price_ht,
        total_ht_before_discount,
        discount_amount: total_ht_before_discount - total_ht,
        total_ht,
        total_vat,
        total_ttc,
    }
}

pub fn sum(values: &[Decimal]) -> Decimal {
    round_money(values.iter().copied().sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn money(s: &str) -> Decimal {
        round_money(dec(s))
    }

    #[test]
    fn arrondi_demi_hors_de_zero() {
        assert_eq!(money("0.125"), money("0.13"));
        assert_eq!(money("-0.125"), money("-0.13"));
        assert_eq!(money("2.675"), money("2.68"));
        assert_eq!(money("1.004"), money("1.00"));
        assert_eq!(money("1.005"), money("1.01"));
    }

    #[test]
    fn ligne_simple() {
        let t = line_totals(&LineInput::new("10", "100.00", "0", "20"));
        assert_eq!(t.total_ht, money("1000.00"));
        assert_eq!(t.total_vat, money("200.00"));
        assert_eq!(t.total_ttc, money("1200.00"));
        assert_eq!(t.discount_amount, money("0.00"));
    }

    #[test]
    fn ligne_avec_remise() {
        let t = line_totals(&LineInput::new("7", "33.33", "5", "20"));
        assert_eq!(t.total_ht_before_discount, money("233.31"));
        assert_eq!(t.total_ht, money("221.64"));
        assert_eq!(t.discount_amount, money("11.67"));
        assert_eq!(t.total_vat, money("44.33"));
        assert_eq!(t.total_ttc, money("265.97"));
    }

    #[test]
    fn base_ttc() {
        let mut i = LineInput::new("1", "120.00", "0", "20").ttc();
        let t = line_totals(&i);
        assert_eq!(t.unit_price_ht, money("100.00"));
        assert_eq!(t.total_ht, money("100.00"));
        assert_eq!(t.total_vat, money("20.00"));
        assert_eq!(t.total_ttc, money("120.00"));

        // TVA non percue sur une base TTC : le prix saisi est deja HT (il n'y a
        // pas de taxe a extraire), donc HT = TTC = 120. Verifie par l'oracle :
        // `calcul_price_total(1, 120, 0, 20, 0, 0, 0, 'TTC', 1, 0)` renvoie
        // [120, 20, 120, 120]. Le `total_vat` de 20 est la TVA "comme si elle
        // etait normale" que Dolibarr renvoie malgre `info_bits & 1` (cf. la
        // doc du crate) : ce n'est pas une incoherence de ce crate.
        i = LineInput::new("1", "120.00", "0", "20")
            .ttc()
            .not_collected();
        let t = line_totals(&i);
        assert_eq!(t.unit_price_ht, money("120.00"));
        assert_eq!(t.total_ht, money("120.00"));
        assert_eq!(t.total_vat, money("20.00"));
        assert_eq!(t.total_ttc, money("120.00"));
    }

    #[test]
    fn quantite_decimale_et_taux_reduit() {
        // 1,5 x 19,99 = 29,985 -> 29,99 ; TTC = 29,985 x 1,10 = 32,9835 -> 32,98 ;
        // TVA = TTC - HT = 2,99. Calculer 10 % de 29,99 donnerait 3,00 et
        // casserait l'egalite HT + TVA = TTC : Dolibarr procede par difference,
        // l'oracle confirme 29,99 / 2,99 / 32,98.
        let t = line_totals(&LineInput::new("1.5", "19.99", "0", "10"));
        assert_eq!(t.total_ht, money("29.99"));
        assert_eq!(t.total_vat, money("2.99"));
        assert_eq!(t.total_ttc, money("32.98"));
        assert_eq!(t.total_ht + t.total_vat, t.total_ttc);
    }

    #[test]
    fn remise_totale() {
        let t = line_totals(&LineInput::new("3", "50.00", "100", "20"));
        assert_eq!(t.total_ht, money("0.00"));
        assert_eq!(t.total_vat, money("0.00"));
        assert_eq!(t.total_ttc, money("0.00"));
    }

    #[test]
    fn total_ttc_egal_ht_plus_tva_sur_10_mille_cas() {
        let mut qty = dec("0.10");
        let mut pu = dec("0.01");
        let mut rem = dec("0");
        for step in 0..10_000u32 {
            let t = line_totals(&LineInput::new(
                &qty.to_string(),
                &pu.to_string(),
                &rem.to_string(),
                if step % 3 == 0 { "20" } else { "5.5" },
            ));
            assert_eq!(
                t.total_ttc,
                round_money(t.total_ht + t.total_vat),
                "ecart au centime pour qty={qty} pu={pu} rem={rem}"
            );
            assert_eq!(t.total_ht + t.discount_amount, t.total_ht_before_discount);
            qty += dec("1.37");
            pu += dec("3.91");
            if rem < dec("90") {
                rem += dec("0.7");
            }
        }
    }

    #[test]
    fn somme_de_lignes_arrondie_une_seule_fois() {
        let lignes = [
            LineInput::new("1", "0.005", "0", "20"),
            LineInput::new("1", "0.005", "0", "20"),
            LineInput::new("1", "0.005", "0", "20"),
        ];
        let totaux: Vec<Decimal> = lignes.iter().map(|l| line_totals(l).total_ht).collect();
        assert_eq!(totaux, vec![money("0.01"), money("0.01"), money("0.01")]);
        assert_eq!(sum(&totaux), money("0.03"));
    }

    #[test]
    fn tenter_lit_un_montant_francais() {
        assert_eq!(tenter("12,50").unwrap(), dec("12.50"));
        assert_eq!(tenter("1 234,56").unwrap(), dec("1234.56"));
        assert_eq!(tenter(" 1\u{00a0}200,00 ").unwrap(), dec("1200.00"));
        assert_eq!(tenter("120.50").unwrap(), dec("120.50"));
        assert_eq!(tenter("12,50\u{20ac}").unwrap(), dec("12.50"));
    }

    #[test]
    fn tenter_refuse_au_lieu_de_paniquer() {
        assert!(tenter("").is_err());
        assert!(tenter("   ").is_err());
        assert!(tenter("abc").is_err());
        assert!(tenter("12,50,00").is_err());
        // Le message nomme la valeur : on sait quoi corriger.
        assert!(tenter("abc").unwrap_err().contains("abc"));
    }

    #[test]
    fn max_montant_vaut_un_billion() {
        assert_eq!(MAX_MONTANT, dec("1000000000000"));
        assert_eq!(-MAX_MONTANT, dec("-1000000000000"));
    }
}
