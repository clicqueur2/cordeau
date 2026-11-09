//! Carnet de commandes : ce qui est vendu et pas encore facture.
//!
//! Une commande validee (`fk_statut = 1`) est un engagement du client ; livree
//! (`fk_statut = 2`), elle attend sa facture. Le carnet somme les deux et
//! signale lesquelles portent deja une facture — sans quoi on compterait deux
//! fois le meme euro.
//!
//! Le lien commande -> facture n'est pas dans `llx_commande` : Dolibarr le range
//! dans `llx_element_element` (`sourcetype = 'commande'`, `targettype =
//! 'facture'`). C'est la seule source de verite, et elle est lue telle quelle.
//!
//! La marge par chantier se lit sur les deux journaux : ce qui a ete facture au
//! client d'un cote, ce que les fournisseurs ont facture de l'autre. Les deux
//! portent `fk_projet` : c'est le seul champ qui les reunisse.

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;

use crate::commande::{STATUT_LIVREE, STATUT_VALIDEE};

/// Une commande ouverte, avec son etat de facturation.
#[derive(Debug, Clone, PartialEq)]
pub struct LigneCarnet {
    pub rowid: i64,
    pub ref_: String,
    pub tiers: Option<String>,
    pub date: Option<String>,
    pub statut: i16,
    pub total_ht: Decimal,
    pub total_ttc: Decimal,
    /// Une facture (hors avoir) pointe deja sur cette commande.
    pub facturee: bool,
}

impl LigneCarnet {
    /// Livree mais pas facturee : c'est elle qui appelle un geste.
    pub fn a_facturer(&self) -> bool {
        self.statut == STATUT_LIVREE && !self.facturee
    }

    /// Le mois `AAAA-MM`, ou une chaine vide si la date manque.
    pub fn mois(&self) -> String {
        self.date
            .as_deref()
            .filter(|d| d.len() >= 7)
            .map(|d| d[..7].to_string())
            .unwrap_or_default()
    }
}

fn opt_s(row: &mysql::Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &mysql::Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &mysql::Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// Commandes validees ou livrees de l'entite, de la plus ancienne a la plus
/// recente — un carnet se lit dans l'ordre ou il s'execute.
pub fn ouvertes(pool: &Pool, entity: i64) -> Result<Vec<LigneCarnet>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT c.rowid, c.ref, s.nom, DATE_FORMAT(c.date_commande, '%Y-%m-%d'), \
                c.fk_statut, CAST(c.total_ht AS CHAR), CAST(c.total_ttc AS CHAR), \
                (SELECT COUNT(*) FROM llx_element_element e \
                  WHERE e.sourcetype = 'commande' AND e.fk_source = c.rowid \
                    AND e.targettype = 'facture') \
         FROM llx_commande c \
         LEFT JOIN llx_societe s ON s.rowid = c.fk_soc \
         WHERE c.entity = :entity AND c.fk_statut IN (:v, :l) \
         ORDER BY c.date_commande ASC, c.rowid ASC",
        params! { "entity" => entity, "v" => STATUT_VALIDEE, "l" => STATUT_LIVREE },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| LigneCarnet {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            tiers: opt_s(&r, 2),
            date: opt_s(&r, 3),
            statut: opt_i(&r, 4).unwrap_or(0) as i16,
            total_ht: opt_dec(&r, 5).unwrap_or_default(),
            total_ttc: opt_dec(&r, 6).unwrap_or_default(),
            facturee: opt_i(&r, 7).unwrap_or(0) > 0,
        })
        .collect())
}

/// Le carnet mois par mois, en TTC, du plus ancien au plus recent.
///
/// Calcule a partir de la meme liste que l'ecran : un total qui ne serait pas
/// la somme des lignes affichees serait un mensonge de plus.
pub fn par_mois(lignes: &[LigneCarnet]) -> Vec<(String, Decimal)> {
    let mut totaux: BTreeMap<String, Decimal> = BTreeMap::new();
    for l in lignes {
        let m = l.mois();
        if m.is_empty() {
            continue;
        }
        *totaux.entry(m).or_insert(Decimal::ZERO) += l.total_ttc;
    }
    totaux.into_iter().collect()
}

/// Total TTC du carnet.
pub fn total(lignes: &[LigneCarnet]) -> Decimal {
    lignes.iter().map(|l| l.total_ttc).sum()
}

/// Ce qui est livre et pas encore facture : le geste du jour.
pub fn a_facturer(lignes: &[LigneCarnet]) -> Decimal {
    lignes
        .iter()
        .filter(|l| l.a_facturer())
        .map(|l| l.total_ttc)
        .sum()
}

/// Marge d'un chantier : facture au client moins facture par les fournisseurs.
#[derive(Debug, Clone, PartialEq)]
pub struct MargeChantier {
    pub projet: i64,
    pub titre: String,
    pub ref_: Option<String>,
    pub ca: Decimal,
    pub achats: Decimal,
    pub facture: bool,
}

impl MargeChantier {
    pub fn marge(&self) -> Decimal {
        self.ca - self.achats
    }

    /// Taux de marge en pourcentage, ou `None` quand rien n'est facture.
    pub fn taux(&self) -> Option<Decimal> {
        if self.ca <= Decimal::ZERO {
            None
        } else {
            Some(money::round_money(
                self.marge() * Decimal::from(100) / self.ca,
            ))
        }
    }
}

/// Les chantiers qui portent a la fois des ventes et des achats, tries par
/// marge decroissante. Un chantier sans achat n'a pas de marge : il a un CA.
pub fn marge_par_chantier(pool: &Pool, entity: i64) -> Result<Vec<MargeChantier>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    let ventes: Vec<(i64, String, Option<String>, Option<String>)> = conn.exec(
        "SELECT p.rowid, p.title, p.ref, CAST(SUM(f.total_ttc) AS CHAR) \
         FROM llx_facture f \
         JOIN llx_projet p ON p.rowid = f.fk_projet \
         WHERE f.entity = :entity AND f.fk_statut >= 1 AND f.fk_projet IS NOT NULL \
           AND (f.type IS NULL OR f.type <> 2) \
         GROUP BY p.rowid, p.title, p.ref",
        params! { "entity" => entity },
    )?;

    let achats: Vec<(i64, Option<String>)> = conn.exec(
        "SELECT fk_projet, CAST(SUM(total_ttc) AS CHAR) FROM llx_facture_fourn \
         WHERE entity = :entity AND fk_statut >= 1 AND fk_projet IS NOT NULL \
         GROUP BY fk_projet",
        params! { "entity" => entity },
    )?;

    let mut par_projet: BTreeMap<i64, (String, Option<String>, Decimal)> = BTreeMap::new();
    for (id, titre, reference, montant) in ventes {
        let ca = montant
            .and_then(|s| Decimal::from_str(s.trim()).ok())
            .unwrap_or_default();
        par_projet.insert(id, (titre, reference, ca));
    }

    let mut achats_par_projet: BTreeMap<i64, Decimal> = BTreeMap::new();
    for (id, montant) in achats {
        let m = montant
            .and_then(|s| Decimal::from_str(s.trim()).ok())
            .unwrap_or_default();
        *achats_par_projet.entry(id).or_default() += m;
    }

    let mut liste: Vec<MargeChantier> = par_projet
        .into_iter()
        .map(|(projet, (titre, ref_, ca))| {
            let achats = achats_par_projet.get(&projet).copied().unwrap_or_default();
            MargeChantier {
                projet,
                titre,
                ref_,
                ca,
                achats,
                facture: true,
            }
        })
        .collect();
    liste.sort_by(|a, b| b.marge().cmp(&a.marge()));
    Ok(liste)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(ref_: &str, date: &str, statut: i16, ttc: &str, facturee: bool) -> LigneCarnet {
        LigneCarnet {
            rowid: 1,
            ref_: ref_.into(),
            tiers: None,
            date: Some(date.into()),
            statut,
            total_ht: Decimal::from_str("0").unwrap(),
            total_ttc: Decimal::from_str(ttc).unwrap(),
            facturee,
        }
    }

    #[test]
    fn le_carnet_se_range_par_mois_dans_l_ordre() {
        let lignes = vec![
            ligne(
                "CO2609-0001",
                "2026-09-14",
                STATUT_VALIDEE,
                "1200.00",
                false,
            ),
            ligne("CO2610-0002", "2026-10-02", STATUT_VALIDEE, "800.00", true),
            ligne("CO2610-0003", "2026-10-20", STATUT_LIVREE, "400.00", false),
        ];
        let mois = par_mois(&lignes);
        assert_eq!(mois.len(), 2);
        assert_eq!(mois[0].0, "2026-09");
        assert_eq!(mois[0].1, Decimal::from_str("1200.00").unwrap());
        assert_eq!(mois[1].0, "2026-10");
        assert_eq!(mois[1].1, Decimal::from_str("1200.00").unwrap());
        assert_eq!(total(&lignes), Decimal::from_str("2400.00").unwrap());
    }

    #[test]
    fn seule_une_livree_non_facturee_appelle_un_geste() {
        let lignes = vec![
            ligne("CO1", "2026-10-01", STATUT_VALIDEE, "500", false),
            ligne("CO2", "2026-10-02", STATUT_LIVREE, "300", false),
            ligne("CO3", "2026-10-03", STATUT_LIVREE, "200", true),
        ];
        assert_eq!(a_facturer(&lignes), Decimal::from_str("300").unwrap());
        assert!(
            !lignes[0].a_facturer(),
            "une commande validee n'est pas livree"
        );
        assert!(lignes[1].a_facturer());
        assert!(!lignes[2].a_facturer(), "deja facturee");
    }

    #[test]
    fn la_marge_se_deduit_du_ca_et_des_achats() {
        let m = MargeChantier {
            projet: 12,
            titre: "Reprise piscine".into(),
            ref_: Some("CH2609-0004".into()),
            ca: Decimal::from_str("10000").unwrap(),
            achats: Decimal::from_str("7000").unwrap(),
            facture: true,
        };
        assert_eq!(m.marge(), Decimal::from_str("3000").unwrap());
        assert_eq!(m.taux(), Some(Decimal::from_str("30").unwrap()));
    }

    #[test]
    fn un_chantier_sans_ca_n_a_pas_de_taux() {
        let m = MargeChantier {
            projet: 13,
            titre: "Etude".into(),
            ref_: None,
            ca: Decimal::ZERO,
            achats: Decimal::from_str("120").unwrap(),
            facture: false,
        };
        assert_eq!(m.taux(), None);
        assert_eq!(m.marge(), Decimal::from_str("-120").unwrap());
    }

    #[test]
    fn une_ligne_sans_date_ne_casse_pas_le_regroupement() {
        let mut l = ligne("CO9", "2026-10-01", STATUT_VALIDEE, "10", false);
        l.date = None;
        assert_eq!(par_mois(&[l]), vec![]);
    }
}
