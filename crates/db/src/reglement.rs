//! Conditions de reglement (`llx_c_payment_term`) et calcul d'echeance.
//!
//! L'echeance est calculee comme dans Dolibarr : on ajoute `nbjour` jours (et
//! le decalage `decalage`) a la date du document, puis, si le code se termine
//! par `ENDMONTH`, on se cale sur le dernier jour du mois obtenu.
//!
//! L'arithmetique de dates est faite ici, en jours depuis l'epoque, pour rester
//! deterministe et testable sans dependance supplementaire.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une condition de reglement, telle qu'utilisee pour le calcul.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub rowid: i64,
    pub code: String,
    /// Libelle du devis et des autres documents (`llx_c_payment_term.libelle`).
    pub libelle: String,
    /// Libelle propre a la **facture** (`libelle_facture`).
    ///
    /// Dolibarr separe les deux : sur un devis on lit « Paiement integral a la
    /// signature », sur la facture « Le reglement est du a reception de la
    /// facture, par virement ». Imprimer le libelle du devis sur la facture
    /// annonce au client une echeance qui n'est pas la sienne.
    pub libelle_facture: String,
    pub nbjour: i64,
    pub decalage: i64,
    /// Pourcentage d'acompte attache a la condition, si le dictionnaire en fixe un.
    pub deposit_percent: Option<String>,
}

impl Condition {
    /// Libelle francais, lisible par le client.
    ///
    /// Les dictionnaires installes par Dolibarr sont en anglais (la traduction
    /// se fait a l'affichage dans le cœur PHP, pas en base) : on la refait ici
    /// pour que le PDF reste francais quel que soit le dictionnaire.
    pub fn libelle_fr(&self) -> String {
        self.nettoyer(self.libelle.clone())
    }

    /// Le libelle a imprimer sur une **facture**.
    ///
    /// Dolibarr distingue `libelle` (devis) et `libelle_facture`. Sur la
    /// production, la condition 83 dit « Paiement integral a la signature »
    /// pour un devis, et « Le reglement est du a reception de la facture, par
    /// virement » pour la facture. Imprimer le premier sur la seconde annonce
    /// au client une echeance qui n'est pas la sienne.
    pub fn libelle_facture_fr(&self) -> String {
        if self.libelle_facture.trim().is_empty() {
            self.libelle_fr()
        } else {
            self.nettoyer(self.libelle_facture.clone())
        }
    }

    fn nettoyer(&self, brut: String) -> String {
        let brut = match self.code.as_str() {
            "RECEP" => "À réception".to_string(),
            "PT_ORDER" => "À la commande".to_string(),
            "PT_DELIVERY" => "À la livraison".to_string(),
            "PT_5050" => "50 % à la commande, 50 % à la livraison".to_string(),
            "DEP30PCTDEL" => {
                let pct = self
                    .deposit_percent
                    .clone()
                    .unwrap_or_else(|| "30".to_string());
                format!("{pct} % d'acompte, solde à la livraison")
            }
            code if code.starts_with("30D") => "30 jours".to_string(),
            code if code.starts_with("60D") => "60 jours".to_string(),
            code if code.starts_with("45D") => "45 jours".to_string(),
            code if code.starts_with("14D") => "14 jours".to_string(),
            code if code.starts_with("10D") => "10 jours".to_string(),
            _ => brut,
        };
        let brut = if self.code.ends_with("ENDMONTH") && !brut.contains("fin de mois") {
            format!("{brut} fin de mois")
        } else {
            brut
        };
        // Un libelle mal saisi ne doit pas sortir casse sur une facture : on
        // retire les fragments orphelins laisses par un modele a trous.
        brut.replace("__DEPOSIT_PERCENT__", "30")
            .replace(", soit %)", "")
            .replace("soit %)", "")
            .replace("( )", "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Une ligne brute du dictionnaire : `rowid`, `code`, `libelle`,
/// `libelle_facture`, `nbjour`, `decalage`, `deposit_percent`.
type LigneDico = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i16>,
    Option<i16>,
    Option<String>,
);

/// Conditions actives de l'entite, dans l'ordre du dictionnaire.
pub fn conditions(pool: &Pool, entity: i64) -> Result<Vec<Condition>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let lignes: Vec<LigneDico> = conn.exec(
        "SELECT rowid, code, libelle, libelle_facture, nbjour, decalage, deposit_percent \
             FROM llx_c_payment_term \
             WHERE active = 1 AND (entity = :entity OR entity = 0) \
             ORDER BY sortorder ASC, rowid ASC",
        params! { "entity" => entity },
    )?;
    Ok(lignes.into_iter().map(depuis).collect())
}

/// Une ligne du dictionnaire, quelle que soit la societe qui la porte.
fn depuis(ligne: LigneDico) -> Condition {
    let (rowid, code, libelle, facture, nbjour, decalage, acompte) = ligne;
    Condition {
        rowid,
        code: code.unwrap_or_default(),
        libelle: match (libelle.clone(), facture.clone()) {
            (Some(l), _) if !l.trim().is_empty() => l,
            (_, Some(f)) => f,
            _ => String::new(),
        },
        libelle_facture: match (facture, libelle) {
            (Some(f), _) if !f.trim().is_empty() => f,
            (_, Some(l)) => l,
            _ => String::new(),
        },
        nbjour: nbjour.unwrap_or(0) as i64,
        decalage: decalage.unwrap_or(0) as i64,
        deposit_percent: acompte.filter(|d| !d.trim().is_empty()),
    }
}

/// Une condition designee par son `rowid`, **quelle que soit sa societe**.
///
/// Un document porte un `fk_cond_reglement` : c'est une reference, pas une
/// saisie. Sur la base de production, la condition 83 appartient a l'entite 1
/// et les factures de l'entite 7 la referencent. Filtrer par entite ici faisait
/// disparaitre l'echeance du PDF — une facture sans condition de reglement.
pub fn condition_par_id(pool: &Pool, rowid: i64) -> Result<Option<Condition>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ligne: Option<LigneDico> = conn.exec_first(
        "SELECT rowid, code, libelle, libelle_facture, nbjour, decalage, deposit_percent \
             FROM llx_c_payment_term WHERE rowid = :rowid LIMIT 1",
        params! { "rowid" => rowid },
    )?;
    Ok(ligne.map(depuis))
}

/// Cree une condition de reglement dans le dictionnaire.
///
/// `code` est l'identifiant stable (majuscules, sans espace), le libelle est ce
/// que le client lit sur le document. Le code doit etre unique par entite
/// (`uk_c_payment_term_code`).
pub fn creer_condition(
    pool: &Pool,
    entity: i64,
    code: &str,
    libelle: &str,
    nbjour: i64,
    fin_de_mois: bool,
) -> Result<i64, Box<dyn Error>> {
    let code = code.trim().to_uppercase().replace(' ', "");
    if code.is_empty() {
        return Err("le code de la condition est obligatoire".into());
    }
    if libelle.trim().is_empty() {
        return Err("le libelle de la condition est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let deja: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_c_payment_term WHERE entity = :entity AND code = :code",
        params! { "entity" => entity, "code" => code.as_str() },
    )?;
    if deja.is_some() {
        return Err(format!("le code {code} existe deja dans le dictionnaire").into());
    }
    // Un code en ...ENDMONTH porte la marque « fin de mois » : c'est elle que
    // lit le calcul d'echeance.
    let code_final = if fin_de_mois {
        format!("{code}ENDMONTH")
    } else {
        code
    };
    let tri: i64 = conn
        .exec_first(
            "SELECT COALESCE(MAX(sortorder), 0) + 10 FROM llx_c_payment_term",
            (),
        )?
        .unwrap_or(10);
    conn.exec_drop(
        "INSERT INTO llx_c_payment_term \
         (entity, code, sortorder, active, libelle, libelle_facture, type_cdr, nbjour) \
         VALUES (:entity, :code, :tri, 1, :libelle, :libelle, :type_cdr, :nbjour)",
        params! {
            "entity" => entity,
            "code" => code_final.as_str(),
            "tri" => tri,
            "libelle" => libelle.trim(),
            "type_cdr" => if fin_de_mois { 1 } else { 0 },
            "nbjour" => nbjour,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Desactive une condition de reglement (on ne supprime jamais une ligne du
/// dictionnaire : des documents passes y font reference).
pub fn desactiver_condition(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_c_payment_term SET active = 0 WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => rowid },
    )?;
    Ok(())
}

/// Une condition precise, ou `None` si elle n'existe pas pour l'entite.
pub fn condition(
    pool: &Pool,
    entity: i64,
    rowid: i64,
) -> Result<Option<Condition>, Box<dyn Error>> {
    if let Some(c) = conditions(pool, entity)?
        .into_iter()
        .find(|c| c.rowid == rowid)
    {
        return Ok(Some(c));
    }
    condition_par_id(pool, rowid)
}

/// Premier jour de la periode de reglement : la condition la plus courante.
pub fn condition_par_defaut(pool: &Pool, entity: i64) -> Result<Option<Condition>, Box<dyn Error>> {
    Ok(conditions(pool, entity)?.into_iter().next())
}

// ---------------------------------------------------------------------------
// Arithmetique de dates (calendrier gregorien proleptique)
// ---------------------------------------------------------------------------

/// Jours ecoules depuis le 1970-01-01 pour une date civile.
fn jours_depuis_civil(annee: i64, mois: u32, jour: u32) -> i64 {
    let a = if mois <= 2 { annee - 1 } else { annee };
    let ere = if a >= 0 { a } else { a - 399 } / 400;
    let a_ere = a - ere * 400;
    let m = (mois as i64 + 9) % 12;
    let jour_annee = (153 * m + 2) / 5 + jour as i64 - 1;
    let jour_ere = a_ere * 365 + a_ere / 4 - a_ere / 100 + jour_annee;
    ere * 146_097 + jour_ere - 719_468
}

/// Date civile correspondant a un nombre de jours depuis le 1970-01-01.
pub fn civil_depuis_jours_public(z: i64) -> (i64, u32, u32) {
    civil_depuis_jours(z)
}

fn civil_depuis_jours(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let ere = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let z_ere = z - ere * 146_097;
    let a = (z_ere - z_ere / 1460 + z_ere / 36_524 - z_ere / 146_096) / 365;
    let jour_annee = z_ere - (365 * a + a / 4 - a / 100);
    let mp = (5 * jour_annee + 2) / 153;
    let jour = (jour_annee - (153 * mp + 2) / 5 + 1) as u32;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let annee = a + ere * 400 + if mois <= 2 { 1 } else { 0 };
    (annee, mois, jour)
}

fn jours_dans_le_mois(annee: i64, mois: u32) -> u32 {
    match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (annee % 4 == 0 && annee % 100 != 0) || annee % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// Lit une date `AAAA-MM-JJ` (les dix premiers caracteres sont suffisants).
pub fn lire_date(iso: &str) -> Option<(i64, u32, u32)> {
    let t: &str = iso.trim();
    if t.len() < 10 {
        return None;
    }
    let annee = t.get(0..4)?.parse().ok()?;
    let mois = t.get(5..7)?.parse().ok()?;
    let jour = t.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&mois) || !(1..=31).contains(&jour) {
        return None;
    }
    Some((annee, mois, jour))
}

/// Ajoute `n` jours a une date `AAAA-MM-JJ`.
pub fn ajouter_jours(iso: &str, n: i64) -> Option<String> {
    let (a, m, j) = lire_date(iso)?;
    let (a2, m2, j2) = civil_depuis_jours(jours_depuis_civil(a, m, j) + n);
    Some(format!("{a2:04}-{m2:02}-{j2:02}"))
}

/// Dernier jour du mois d'une date `AAAA-MM-JJ`.
pub fn fin_de_mois(iso: &str) -> Option<String> {
    let (a, m, _) = lire_date(iso)?;
    Some(format!("{a:04}-{m:02}-{:02}", jours_dans_le_mois(a, m)))
}

/// Echeance d'un document : date du document + `nbjour` + `decalage`, puis
/// dernier jour du mois si la condition est de type `...ENDMONTH`.
pub fn echeance(date_document: &str, code: &str, nbjour: i64, decalage: i64) -> Option<String> {
    let decale = ajouter_jours(date_document, nbjour + decalage)?;
    if code.to_ascii_uppercase().ends_with("ENDMONTH") {
        fin_de_mois(&decale)
    } else {
        Some(decale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trente_jours_ajoute_trente_jours() {
        assert_eq!(
            echeance("2026-01-15", "30D", 30, 0).as_deref(),
            Some("2026-02-14")
        );
    }

    #[test]
    fn trente_jours_fin_de_mois_retombe_sur_le_dernier_jour() {
        assert_eq!(
            echeance("2026-01-15", "30DENDMONTH", 30, 0).as_deref(),
            Some("2026-02-28")
        );
    }

    #[test]
    fn fin_de_mois_gere_les_annees_bissextiles() {
        assert_eq!(
            echeance("2028-01-15", "30DENDMONTH", 30, 0).as_deref(),
            Some("2028-02-29")
        );
    }

    #[test]
    fn fin_de_mois_passe_l_annee() {
        assert_eq!(
            echeance("2026-12-15", "30DENDMONTH", 30, 0).as_deref(),
            Some("2027-01-31")
        );
    }

    #[test]
    fn a_reception_est_le_lendemain() {
        assert_eq!(
            echeance("2026-03-10", "RECEP", 1, 0).as_deref(),
            Some("2026-03-11")
        );
    }

    #[test]
    fn soixante_jours_fin_de_mois_traverse_deux_mois() {
        // 31/01 + 60 j = 01/04 (2026 non bissextile) -> fin avril.
        assert_eq!(
            echeance("2026-01-31", "60DENDMONTH", 60, 0).as_deref(),
            Some("2026-04-30")
        );
    }

    #[test]
    fn le_decalage_s_ajoute_au_nombre_de_jours() {
        assert_eq!(
            echeance("2026-06-01", "30D", 30, 5).as_deref(),
            Some("2026-07-06")
        );
    }

    #[test]
    fn une_date_invalide_ne_produit_rien() {
        assert!(echeance("pas une date", "30D", 30, 0).is_none());
    }

    fn condition(libelle: &str, facture: &str) -> Condition {
        Condition {
            rowid: 83,
            code: "PA100SIG".to_string(),
            libelle: libelle.to_string(),
            libelle_facture: facture.to_string(),
            nbjour: 0,
            decalage: 0,
            deposit_percent: None,
        }
    }

    /// Le cas de la production : le devis et la facture n'annoncent pas la meme
    /// echeance pour la meme condition.
    #[test]
    fn la_facture_porte_son_propre_libelle() {
        let c = condition(
            "Paiement intégral à la signature",
            "Le règlement est dû à réception de la facture, par virement bancaire.",
        );
        assert!(c.libelle_fr().contains("signature"));
        assert!(c.libelle_facture_fr().contains("réception de la facture"));
    }

    /// Sans libelle de facture, on retombe sur celui du devis : mieux vaut un
    /// libelle approximatif qu'une ligne vide.
    #[test]
    fn sans_libelle_de_facture_on_garde_celui_du_devis() {
        let c = condition("Paiement intégral à la signature", "   ");
        assert_eq!(c.libelle_facture_fr(), c.libelle_fr());
    }
}
