//! Ecriture d'une facture d'achat quand la TVA n'est pas deductible.
//!
//! `crate::ecritures` verse **toute** la TVA d'un achat en 445660 « TVA
//! deductible ». C'est juste tant que la TVA est effectivement recuperable. Des
//! qu'une ligne porte le drapeau `tva_npr` (`info_bits & 1`, cf.
//! `crate::tva_npr`), ce versement fabrique une creance de TVA qui n'existe pas :
//! on recupere une TVA qu'on n'a pas le droit de recuperer. Ce module separe les
//! deux TVA et pose la bonne ecriture.
//!
//! # Pourquoi la TVA non deductible s'ajoute au HT en charge
//!
//! Une TVA non deductible n'est pas une creance sur l'Etat : il n'y a aucun
//! credit de TVA a reporter en 44566, et la declarer en TVA deductible est un
//! motif de redressement. Elle fait partie du **cout d'acquisition** du bien ou
//! du service : c'est une charge de plus, au meme titre que le prix hors taxes.
//! Le debit du compte de charge vaut donc `HT + TVA non deductible`.
//!
//! # Pourquoi la meme TVA s'ajoute au TTC en dette
//!
//! Le fournisseur est du de la **totalite**, TVA non deductible comprise :
//! l'entreprise la lui paie, elle ne peut simplement pas la recuperer. Or le
//! TTC du document Dolibarr **l'exclut** : `calcul_price_total` neutralise le
//! taux des que `info_bits & 1` est pose, pour le total TTC
//! (`price.lib.php:308` : `(($info_bits & 1) ? 0 : $txtva)`, et de meme lignes
//! 303, 313, 319, 324, 329) alors que le montant de TVA, lui, reste calcule au
//! taux plein (`$result[1]`, ligne 310). Sur une facture entierement NPR on a
//! donc `total_ttc = total_ht`, et sur une facture mixte le TTC n'inclut que la
//! TVA des lignes normales. Le credit de 401000 vaut donc
//! `TTC du document + TVA non deductible` : c'est le seul moyen de retrouver ce
//! qu'on doit vraiment au fournisseur.
//!
//! Consequence assumee a ne pas confondre avec un bug : **le TTC ecrit n'est pas
//! le TTC du document** des qu'il y a de la TVA non deductible. L'ecart vaut
//! exactement la TVA NPR.
//!
//! # Pourquoi l'ecriture tient quand meme
//!
//! | compte | sens | montant |
//! |---|---|---|
//! | 601000 Achats | debit | `HT + TVA non deductible` |
//! | 445660 TVA deductible | debit | `TVA deductible` seule |
//! | 401000 Fournisseurs | credit | `TTC + TVA non deductible` |
//!
//! Comme `TTC = HT + TVA deductible` (c'est la definition que pose `ventiler`,
//! en suivant Dolibarr), le debit total vaut `HT + TVA non deductible + TVA
//! deductible` et le credit `HT + TVA deductible + TVA non deductible` : les
//! deux cotes sont le meme nombre, au centime. Il n'y a pas d'ajustement, pas de
//! ligne d'equilibrage, pas d'arrondi arbitraire.
//!
//! Une TVA non deductible a 0 (le cas courant : la ligne NPR porte un taux, mais
//! le fournisseur ne facture pas de TVA) ne produit aucun ecart : le HT part en
//! charge et le TTC en dette, comme une facture ordinaire.
//!
//! # Ce que ce module ne fait pas
//!
//! - il ne lit pas la base : il prend une ventilation deja calculee, ce qui le
//!   rend testable sans MySQL ;
//! - il ne verifie pas que le TTC de la ventilation vaut celui de l'en-tete de
//!   la facture : c'est le role de l'appelant, qui a le document sous la main
//!   (`crate::ecritures` le fait deja pour les factures sans NPR) ;
//! - il n'ecrit rien : il rend une `Ecriture`, `comptabilite::verifier` tranche ;
//! - il ne touche pas aux ventes : sur une vente, une TVA non percue se traduit
//!   par une TVA collectee nulle, pas par une charge.

use comptabilite::{Ecriture, Ligne};
use rust_decimal::Decimal;

use crate::ecritures::{COMPTE_ACHATS, COMPTE_FOURNISSEURS, COMPTE_TVA_DEDUCTIBLE, JOURNAL_ACHATS};

/// Les quatre montants d'une facture d'achat, TVA separee.
///
/// `ttc` est le TTC **du document**, donc hors TVA non deductible (voir la note
/// du module) : c'est `HT + tva_deductible`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ventilation {
    /// Somme des lignes hors taxes.
    pub ht: Decimal,
    /// TVA recuperable, seule a aller en 445660.
    pub tva_deductible: Decimal,
    /// TVA `tva_npr` : un cout, jamais une creance de TVA.
    pub tva_non_deductible: Decimal,
    /// TTC du document : `ht + tva_deductible`.
    pub ttc: Decimal,
}

/// Sens d'une ligne.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sens {
    Debit,
    Credit,
}

/// Ajoute une ligne, au sens demande, si elle porte un montant.
///
/// Le moteur refuse une ligne dont le debit et le credit valent tous les deux
/// zero (`comptabilite::verifier_ligne`) : une ligne nulle n'est donc pas ecrite
/// du tout, et surtout pas ecrite a zero.
fn pousser(
    out: &mut Vec<Ligne>,
    compte: &str,
    libelle: &str,
    date: &str,
    sens: Sens,
    montant: Decimal,
) {
    let montant = money::round_money(montant);
    if montant.is_zero() {
        return;
    }
    let (debit, credit) = match sens {
        Sens::Debit => (montant, Decimal::ZERO),
        Sens::Credit => (Decimal::ZERO, montant),
    };
    out.push(Ligne {
        compte: compte.to_string(),
        libelle: libelle.to_string(),
        debit,
        credit,
        journal: JOURNAL_ACHATS.to_string(),
        date: date.to_string(),
    });
}

/// Additionne des lignes de facture en separant les deux TVA.
///
/// Entree : `(taux, non_deductible, HT, TVA)`, une entree par ligne de document.
/// Le taux ne sert pas a decouper (c'est le drapeau qui tranche) : il reste dans
/// la signature pour que l'appelant n'ait pas a detacher les colonnes, et parce
/// qu'un futur regroupement par taux en aura besoin.
///
/// Le `ttc` rendu est `ht + tva_deductible`, comme Dolibarr : la TVA non
/// deductible n'entre pas dans le TTC du document (`price.lib.php:308`). Un lot
/// vide rend des zeros, sans erreur : il n'y a rien a ventiler, ce n'est pas une
/// faute.
#[must_use]
pub fn ventiler(lignes: &[(Decimal, bool, Decimal, Decimal)]) -> Ventilation {
    let mut ht = Decimal::ZERO;
    let mut deductible = Decimal::ZERO;
    let mut non_deductible = Decimal::ZERO;
    for (_, npr, ligne_ht, ligne_tva) in lignes {
        ht += *ligne_ht;
        if *npr {
            non_deductible += *ligne_tva;
        } else {
            deductible += *ligne_tva;
        }
    }

    let ht = money::round_money(ht);
    let tva_deductible = money::round_money(deductible);
    Ventilation {
        ht,
        tva_deductible,
        tva_non_deductible: money::round_money(non_deductible),
        ttc: money::round_money(ht + tva_deductible),
    }
}

/// L'ecriture d'une facture d'achat : charge, TVA deductible, dette.
///
/// Debit 601000 de `HT + TVA non deductible`, debit 445660 de la seule TVA
/// deductible (aucune ligne si elle est nulle), credit 401000 de
/// `TTC + TVA non deductible`. Le numero est `AC-<reference>`, comme
/// `crate::ecritures`, pour que l'appelant puisse refuser le doublon
/// (`comptabilite::Erreur::DejaAjoutee`) sans tenir de registre.
///
/// Une ventilation nulle ne produit aucune ligne : il n'y a pas d'ecriture a
/// tenir, et `comptabilite::verifier` la refuse en `EcritureVide` plutot que de
/// laisser passer une ecriture vide.
#[must_use]
pub fn ecriture_achat(facture_ref: &str, date: &str, tiers: &str, vent: &Ventilation) -> Ecriture {
    let reference = facture_ref.trim();
    let tiers = tiers.trim();
    let libelle = if tiers.is_empty() {
        reference.to_string()
    } else {
        format!("{reference} - {tiers}")
    };

    let tva_non_deductible = money::round_money(vent.tva_non_deductible);
    let mut lignes: Vec<Ligne> = Vec::with_capacity(3);
    // La TVA non deductible est un cout : elle grossit la charge.
    pousser(
        &mut lignes,
        COMPTE_ACHATS,
        &libelle,
        date,
        Sens::Debit,
        vent.ht + tva_non_deductible,
    );
    // La TVA deductible seule est une creance sur l'Etat.
    pousser(
        &mut lignes,
        COMPTE_TVA_DEDUCTIBLE,
        &format!("{libelle} - TVA deductible"),
        date,
        Sens::Debit,
        vent.tva_deductible,
    );
    // Le fournisseur est du de la totalite, TVA non deductible comprise.
    pousser(
        &mut lignes,
        COMPTE_FOURNISSEURS,
        &libelle,
        date,
        Sens::Credit,
        vent.ttc + tva_non_deductible,
    );

    let ecriture = Ecriture {
        numero: format!("{JOURNAL_ACHATS}-{reference}"),
        date: date.to_string(),
        libelle,
        lignes,
    };

    // `ventiler` garantit `ttc = ht + tva deductible`, donc les deux cotes sont
    // le meme nombre. Si cela tombe en erreur, c'est que la `Ventilation` a ete
    // fabriquee a la main avec un `ttc` incoherent : c'est un bug de l'appelant,
    // et il doit se voir des les tests, pas dans un grand livre.
    debug_assert_eq!(
        ecriture.lignes.iter().map(|l| l.debit).sum::<Decimal>(),
        ecriture.lignes.iter().map(|l| l.credit).sum::<Decimal>(),
        "ecriture d'achat desequilibree : la ventilation ne verifie pas ttc = ht + tva deductible"
    );
    ecriture
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn d(valeur: &str) -> Decimal {
        Decimal::from_str(valeur).expect("montant de test illisible")
    }

    fn debit(ecriture: &Ecriture, compte: &str) -> Decimal {
        ecriture
            .lignes
            .iter()
            .filter(|l| l.compte == compte)
            .map(|l| l.debit)
            .sum()
    }

    fn credit(ecriture: &Ecriture, compte: &str) -> Decimal {
        ecriture
            .lignes
            .iter()
            .filter(|l| l.compte == compte)
            .map(|l| l.credit)
            .sum()
    }

    fn lignes_du_compte(ecriture: &Ecriture, compte: &str) -> usize {
        ecriture
            .lignes
            .iter()
            .filter(|l| l.compte == compte)
            .count()
    }

    #[test]
    fn facture_ordinaire_la_tva_va_en_445660() {
        // 1 000 HT a 20 %, TVA deductible : la creance de TVA existe.
        let vent = ventiler(&[(d("20"), false, d("1000.00"), d("200.00"))]);
        assert_eq!(
            vent,
            Ventilation {
                ht: d("1000.00"),
                tva_deductible: d("200.00"),
                tva_non_deductible: Decimal::ZERO,
                ttc: d("1200.00"),
            }
        );

        let e = ecriture_achat("FF2611-0001", "2026-11-04", "Cimentier", &vent);
        assert_eq!(e.numero, "AC-FF2611-0001");
        assert_eq!(e.libelle, "FF2611-0001 - Cimentier");
        assert_eq!(debit(&e, "601000"), d("1000.00"));
        assert_eq!(debit(&e, "445660"), d("200.00"));
        assert_eq!(credit(&e, "401000"), d("1200.00"));
        assert_eq!(lignes_du_compte(&e, "601000"), 1);
        assert_eq!(e.lignes.len(), 3);
        assert_eq!(comptabilite::verifier(&e), Ok(()));
    }

    #[test]
    fn tva_non_deductible_va_en_charge_et_pas_en_445660() {
        // 1 000 HT a `20*` : 200 de TVA que l'on n'a pas le droit de recuperer.
        let vent = ventiler(&[(d("20"), true, d("1000.00"), d("200.00"))]);
        // Le TTC du document exclut la TVA NPR (Dolibarr, price.lib.php:308).
        assert_eq!(vent.ttc, d("1000.00"));
        assert_eq!(vent.tva_non_deductible, d("200.00"));
        assert_eq!(vent.tva_deductible, Decimal::ZERO);

        let e = ecriture_achat("FF2611-0002", "2026-11-04", "Cimentier", &vent);
        assert_eq!(
            debit(&e, "601000"),
            d("1200.00"),
            "la charge porte le HT et la TVA non deductible"
        );
        assert_eq!(
            debit(&e, "445660"),
            Decimal::ZERO,
            "aucune TVA recuperable ne doit atterrir en 445660"
        );
        assert_eq!(
            lignes_du_compte(&e, "445660"),
            0,
            "pas de ligne 445660 : le moteur refuse une ligne a zero"
        );
        assert_eq!(
            credit(&e, "401000"),
            d("1200.00"),
            "le fournisseur est du du HT et de la TVA non deductible"
        );
        assert_eq!(e.lignes.len(), 2);
        // Le point qui compte : l'ecriture tient, sans ligne d'ajustement.
        assert_eq!(comptabilite::verifier(&e), Ok(()));
    }

    #[test]
    fn facture_mixte_separe_les_deux_tva() {
        // 500 a 20 % deductible + 500 a `20*` : 100 de TVA recuperable,
        // 100 de TVA en charge.
        let vent = ventiler(&[
            (d("20"), false, d("500.00"), d("100.00")),
            (d("20"), true, d("500.00"), d("100.00")),
        ]);
        assert_eq!(vent.ht, d("1000.00"));
        assert_eq!(vent.tva_deductible, d("100.00"));
        assert_eq!(vent.tva_non_deductible, d("100.00"));
        assert_eq!(
            vent.ttc,
            d("1100.00"),
            "le TTC n'inclut que la TVA deductible"
        );

        let e = ecriture_achat("FF2611-0003", "2026-11-04", "Cimentier", &vent);
        assert_eq!(debit(&e, "601000"), d("1100.00"));
        assert_eq!(debit(&e, "445660"), d("100.00"));
        assert_eq!(credit(&e, "401000"), d("1200.00"));
        assert_eq!(e.lignes.len(), 3);
        assert_eq!(comptabilite::verifier(&e), Ok(()));
    }

    #[test]
    fn ventilation_vide_rend_des_zeros() {
        let vent = ventiler(&[]);
        assert_eq!(vent, Ventilation::default());
        assert!(vent.ht.is_zero());
        assert!(vent.tva_deductible.is_zero());
        assert!(vent.tva_non_deductible.is_zero());
        assert!(vent.ttc.is_zero());

        // Rien a ventiler, donc rien a ecrire : le moteur refuse l'ecriture vide
        // plutot que de laisser passer une coquille vide.
        let e = ecriture_achat("FF2611-0004", "2026-11-04", "", &vent);
        assert!(e.lignes.is_empty());
        assert_eq!(e.libelle, "FF2611-0004", "sans tiers, la reference suffit");
        assert_eq!(
            comptabilite::verifier(&e),
            Err(comptabilite::Erreur::EcritureVide)
        );
    }

    #[test]
    fn la_saisie_vingt_etoile_alimente_la_tva_non_deductible() {
        // Chaine complete : la saisie `20*` donne le drapeau, `ventiler` le
        // range du bon cote, l'ecriture le met en charge.
        let taux = crate::tva_npr::analyser("20*").expect("« 20* » doit etre lu");
        assert!(taux.non_deductible);

        let ht = d("1000.00");
        let tva = money::round_money(ht * taux.taux / Decimal::from(100));
        let vent = ventiler(&[(taux.taux, taux.non_deductible, ht, tva)]);
        assert_eq!(vent.tva_deductible, Decimal::ZERO);
        assert_eq!(vent.tva_non_deductible, d("200.00"));

        let e = ecriture_achat("FF2611-0005", "2026-11-04", "Cimentier", &vent);
        assert_eq!(debit(&e, "601000"), d("1200.00"));
        assert_eq!(credit(&e, "401000"), d("1200.00"));
        assert_eq!(comptabilite::verifier(&e), Ok(()));
    }
}
