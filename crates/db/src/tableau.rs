//! Le tableau de bord d'accueil : les chiffres, et surtout les alertes.
//!
//! Un tableau de bord qui ne fait que compter ne sert a rien : ce qu'on veut
//! savoir en se connectant le matin, c'est **ce qui reclame une decision**. Trois
//! choses, dans cet ordre :
//!
//! 1. les factures en retard — de l'argent qui devait rentrer et qui ne rentre pas ;
//! 2. les devis sans reponse depuis deux semaines — du travail qui s'echappe ;
//! 3. les produits sous le seuil d'alerte — du stock qui va arreter un chantier.
//!
//! Deux calculs sont faits en **une requete** chacun. La version precedente
//! appelait `payment::balance` dans une boucle, ce qui faisait autant
//! d'aller-retours que de factures : invisible sur dix lignes, penible sur mille.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Une ligne brute de facture en retard : `(rowid, ref, tiers, echeance, reste, jours)`.
type LigneRetard = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
);
/// Une ligne brute de devis sans reponse : `(rowid, ref, tiers, date, jours, ttc)`.
type LigneDevis = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
);
/// Une ligne brute de produit sous seuil : `(ref, libelle, stock, seuil)`.
type LigneStock = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Une facture en retard, telle que l'alerte la montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnRetard {
    pub rowid: i64,
    pub reference: String,
    pub tiers: String,
    pub echeance: String,
    pub reste: Decimal,
    /// Jours de retard, jamais negatif.
    pub jours: i64,
}

/// Un devis sans reponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SansReponse {
    pub rowid: i64,
    pub reference: String,
    pub tiers: String,
    pub date: String,
    pub jours: i64,
    pub montant: Decimal,
}

/// Un produit sous son seuil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockBas {
    pub reference: String,
    pub libelle: String,
    pub stock: Decimal,
    pub seuil: Decimal,
}

/// Les totaux de la page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chiffres {
    pub facture: Decimal,
    pub encaisse: Decimal,
    pub reste: Decimal,
    pub retard: Decimal,
    pub nb_factures: i64,
}

fn dec(v: Option<String>) -> Decimal {
    v.as_deref()
        .map(str::trim)
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or_default()
}

/// Les chiffres de l'exercice, en deux requetes.
pub fn chiffres(pool: &Pool, entity: i64) -> Result<Chiffres, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ligne: Option<(i64, Option<String>, Option<String>)> = conn.exec_first(
        "SELECT COUNT(*), CAST(COALESCE(SUM(f.total_ttc), 0) AS CHAR), \
                CAST(COALESCE(SUM(pe.regle), 0) AS CHAR) \
         FROM llx_facture f \
         LEFT JOIN (SELECT fk_facture, SUM(amount) AS regle \
                    FROM llx_paiement_facture GROUP BY fk_facture) pe \
                ON pe.fk_facture = f.rowid \
         WHERE f.entity = :entity AND f.fk_statut >= 1 \
           AND (f.type IS NULL OR f.type <> 2)",
        params! { "entity" => entity },
    )?;
    let (nb, ttc, regle) = ligne.unwrap_or((0, None, None));
    let facture = dec(ttc);
    let encaisse = dec(regle);

    Ok(Chiffres {
        facture,
        encaisse,
        reste: facture - encaisse,
        retard: Decimal::ZERO,
        nb_factures: nb,
    })
}

/// Les factures en retard, la plus ancienne d'abord.
pub fn en_retard(
    pool: &Pool,
    entity: i64,
    aujourd_hui: &str,
    limite: u32,
) -> Result<Vec<EnRetard>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneRetard> = conn.exec(
        "SELECT f.rowid, f.ref, s.nom, \
                DATE_FORMAT(f.date_lim_reglement, '%Y-%m-%d'), \
                CAST(f.total_ttc - COALESCE(pe.regle, 0) AS CHAR), \
                DATEDIFF(:jour, f.date_lim_reglement) \
         FROM llx_facture f \
         LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         LEFT JOIN (SELECT fk_facture, SUM(amount) AS regle \
                    FROM llx_paiement_facture GROUP BY fk_facture) pe \
                ON pe.fk_facture = f.rowid \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.paye = 0 \
           AND f.date_lim_reglement IS NOT NULL AND f.date_lim_reglement < :jour \
         ORDER BY f.date_lim_reglement ASC \
         LIMIT :limite",
        params! { "entity" => entity, "jour" => aujourd_hui, "limite" => limite.clamp(1, 100) },
    )?;

    Ok(rows
        .into_iter()
        .map(
            |(rowid, reference, tiers, echeance, reste, jours)| EnRetard {
                rowid,
                reference: reference.unwrap_or_default(),
                tiers: tiers.unwrap_or_default(),
                echeance: echeance.unwrap_or_default(),
                reste: dec(reste),
                jours: jours.unwrap_or(0).max(0),
            },
        )
        .collect())
}

/// Les devis en attente depuis plus de `jours` jours.
pub fn devis_sans_reponse(
    pool: &Pool,
    entity: i64,
    aujourd_hui: &str,
    jours: i64,
    limite: u32,
) -> Result<Vec<SansReponse>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneDevis> = conn.exec(
        "SELECT p.rowid, p.ref, s.nom, DATE_FORMAT(COALESCE(p.datep, p.datec), '%Y-%m-%d'), \
                    DATEDIFF(:jour, COALESCE(p.datep, p.datec)), \
                    CAST(p.total_ttc AS CHAR) \
             FROM llx_propal p \
             LEFT JOIN llx_societe s ON s.rowid = p.fk_soc \
             WHERE p.entity = :entity AND p.fk_statut = 1 \
               AND DATEDIFF(:jour, COALESCE(p.datep, p.datec)) >= :jours \
             ORDER BY COALESCE(p.datep, p.datec) ASC \
             LIMIT :limite",
        params! {
            "entity" => entity,
            "jour" => aujourd_hui,
            "jours" => jours.max(1),
            "limite" => limite.clamp(1, 100),
        },
    )?;

    Ok(rows
        .into_iter()
        .map(|(rowid, reference, tiers, date, j, montant)| SansReponse {
            rowid,
            reference: reference.unwrap_or_default(),
            tiers: tiers.unwrap_or_default(),
            date: date.unwrap_or_default(),
            jours: j.unwrap_or(0).max(0),
            montant: dec(montant),
        })
        .collect())
}

/// Les produits dont le stock est au seuil ou en dessous, quand un seuil existe.
///
/// Un seuil a zero n'est pas un seuil : il ne declenche rien, sinon tout produit
/// sans seuil alerterait en permanence.
pub fn stock_bas(pool: &Pool, entity: i64) -> Result<Vec<StockBas>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneStock> = conn.exec(
        "SELECT p.ref, p.label, \
                CAST(COALESCE((SELECT SUM(ps.reel) FROM llx_product_stock ps \
                               WHERE ps.fk_product = p.rowid), 0) AS CHAR), \
                CAST(p.seuil_stock_alerte AS CHAR) \
         FROM llx_product p \
         WHERE p.entity = :entity AND p.tosell = 1 \
           AND p.seuil_stock_alerte IS NOT NULL AND p.seuil_stock_alerte > 0 \
         ORDER BY p.ref ASC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(|(reference, libelle, stock, seuil)| StockBas {
            reference: reference.unwrap_or_default(),
            libelle: libelle.unwrap_or_default(),
            stock: dec(stock),
            seuil: dec(seuil),
        })
        .filter(|s| s.stock <= s.seuil)
        .collect())
}

/// Ce que representent les devis : ce qui est en jeu, et ce qui se transforme.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Devis {
    /// Montant des devis encore en attente de reponse.
    pub en_cours: Decimal,
    pub nb_en_cours: i64,
    /// Montant des devis signes sur la periode.
    pub signes: Decimal,
    pub nb_signes: i64,
    /// Montant des devis perdus.
    pub perdus: Decimal,
    /// Taux de transformation en pourcentage, arrondi a l'unite.
    ///
    /// Calcule sur les **montants**, pas sur les nombres : un devis de 50 000 €
    /// signe ne vaut pas un devis de 300 € perdu. Rend 0 quand rien n'a ete
    /// emis, plutot qu'une division par zero.
    pub taux: i64,
}

impl Devis {
    /// Le total emis : signe + perdu + en attente.
    pub fn emis(&self) -> Decimal {
        self.signes + self.perdus + self.en_cours
    }
}

/// Les devis : en attente, signes, perdus, et le taux de transformation.
///
/// `depuis` borne les devis **signes ou perdus** : le taux porte sur une
/// periode, sinon il ne veut rien dire. L'en-cours, lui, est toujours complet :
/// c'est ce qui reste a gagner aujourd'hui.
pub fn devis(pool: &Pool, entity: i64, depuis: &str) -> Result<Devis, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    let en_cours: Option<(i64, Option<String>)> = conn.exec_first(
        "SELECT COUNT(*), CAST(COALESCE(SUM(total_ttc), 0) AS CHAR) \
         FROM llx_propal WHERE entity = :entity AND fk_statut = 0",
        params! { "entity" => entity },
    )?;
    let (nb_en_cours, montant_en_cours) = en_cours.unwrap_or((0, None));

    let signes: Option<(i64, Option<String>)> = conn.exec_first(
        "SELECT COUNT(*), CAST(COALESCE(SUM(total_ttc), 0) AS CHAR) \
         FROM llx_propal \
         WHERE entity = :entity AND fk_statut = 2 AND COALESCE(date_signature, datep, datec) >= :depuis",
        params! { "entity" => entity, "depuis" => depuis },
    )?;
    let (nb_signes, montant_signes) = signes.unwrap_or((0, None));

    let perdus: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(total_ttc), 0) AS CHAR) \
         FROM llx_propal \
         WHERE entity = :entity AND fk_statut = 3 AND COALESCE(date_cloture, datep, datec) >= :depuis",
        params! { "entity" => entity, "depuis" => depuis },
    )?;

    let mut d = Devis {
        en_cours: dec(montant_en_cours),
        nb_en_cours,
        signes: dec(montant_signes),
        nb_signes,
        perdus: dec(perdus),
        taux: 0,
    };

    let emis_periode = d.signes + d.perdus;
    if emis_periode > Decimal::ZERO {
        d.taux = ((d.signes / emis_periode) * Decimal::from(100))
            .round_dp(0)
            .try_into()
            .unwrap_or(0);
    }
    Ok(d)
}

pub fn total_retard(rows: &[EnRetard]) -> Decimal {
    rows.iter().map(|f| f.reste).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_total_en_retard_additionne_les_restes() {
        let f = |reste: &str| EnRetard {
            rowid: 1,
            reference: "F".into(),
            tiers: "C".into(),
            echeance: "2026-01-01".into(),
            reste: dec(Some(reste.to_string())),
            jours: 3,
        };
        assert_eq!(
            total_retard(&[f("100"), f("20.50")]),
            dec(Some("120.50".into()))
        );
        assert_eq!(total_retard(&[]), Decimal::ZERO);
    }

    #[test]
    fn un_montant_illisible_vaut_zero() {
        assert_eq!(dec(None), Decimal::ZERO);
        assert_eq!(dec(Some("".into())), Decimal::ZERO);
    }
}
