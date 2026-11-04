//! Scellé des ventes de caisse : rendre une journée de ventes **vérifiable**.
//!
//! L'article 286-I-3° bis du CGI exige d'un logiciel de caisse quatre
//! propriétés : inaltérabilité, sécurisation, conservation, archivage. Ce
//! module n'en fournit qu'une brique, mais c'est la plus structurante : une
//! **empreinte chaînée** sur les ventes d'une journée, conservée en base.
//!
//! # Le principe
//!
//! Chaque vente entre dans un condensat `SHA-256` qui reprend le condensat
//! précédent. Modifier un montant, supprimer une vente ou en insérer une
//! décale toute la chaîne : le condensat final ne correspond plus au scellé
//! enregistré. C'est ce qu'on appelle un scellé, pas une certification.
//!
//! # Ce que ce module ne fait pas, et qu'il faut dire
//!
//! - Il **détecte** une modification, il ne l'**empêche** pas. Un utilisateur
//!   qui a les droits SQL peut réécrire les ventes *et* le scellé.
//! - Le scellé vit dans `llx_const`, faute de table dédiée (le schéma Dolibarr
//!   ne doit pas bouger). C'est un choix assumé, pas une garantie forte.
//! - Rien n'est signé cryptographiquement : pas de clé privée, donc pas de
//!   valeur probante au-delà de la détection interne.
//!
//! Autrement dit : c'est le premier étage. Il rend une falsification
//! **visible** ; pour la rendre **impossible**, il faudrait un journal en
//! ajout seul et une signature — chantier suivant.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};

/// Ce que le scellé d'une journée contient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scelle {
    /// Date scellée, `AAAA-MM-JJ`.
    pub date: String,
    /// Nombre de ventes entrées dans la chaîne.
    pub ventes: usize,
    /// Total encaissé de la journée, au centime.
    pub total: Decimal,
    /// Condensat final de la chaîne.
    pub empreinte: String,
    /// Condensat enregistré en base au moment du scellé, s'il existait déjà.
    pub empreinte_enregistree: Option<String>,
}

/// Produit une **archive verifiable** d'une journee de ventes.
///
/// Le fichier se suffit a lui-meme : chaque ligne porte la reference, la date,
/// le montant TTC et le condensat de la chaine a cet instant. La derniere ligne
/// reprend le condensat final. On peut donc le conserver dix ans et le
/// re-verifier **sans la base** — c'est la mecanique d'archivage de l'article
/// 286 CGI.
///
/// Une ligne par vente, champs separes par `|` :
///
/// ```text
/// VENTE|POS-20261104-0001|2026-11-04|10.00|<condensat>
/// SCELLE|2026-11-04|<condensat final>|<nombre>|<total>
/// ```
pub fn archive(pool: &Pool, entity: i64, date: &str) -> Result<String, Box<dyn Error>> {
    let ventes = ventes_du_jour(pool, entity, date)?;
    let mut chaine = String::from("GENESE");
    let mut total = Decimal::ZERO;
    let mut sortie = String::new();

    for v in &ventes {
        chaine = empreinte(&chaine, v);
        total += v.total_ttc;
        sortie.push_str(&format!(
            "VENTE|{}|{}|{}|{}\n",
            v.reference, v.date, v.total_ttc, chaine
        ));
    }
    sortie.push_str(&format!(
        "SCELLE|{}|{}|{}|{}\n",
        date,
        chaine,
        ventes.len(),
        total
    ));
    Ok(sortie)
}

/// Resultat de la re-verification d'une archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationArchive {
    pub date: String,
    pub ventes: usize,
    pub total: Decimal,
    /// Vrai quand le condensat recalcule egale celui de la ligne `SCELLE`.
    pub intact: bool,
}

/// Re-verifie une archive **sans acces a la base**.
///
/// Toute ligne alteree, ajoutee ou retiree casse la chaine : le condensat final
/// ne tombe plus juste. C'est le seul controle qui vaille pour une archive
/// conservee hors du systeme.
pub fn verifier_archive(texte: &str) -> Result<VerificationArchive, Box<dyn Error>> {
    let mut chaine = String::from("GENESE");
    let mut total = Decimal::ZERO;
    let mut ventes = 0usize;
    let mut sceau: Option<(String, String, usize, Decimal)> = None;

    for ligne in texte.lines() {
        if ligne.trim().is_empty() {
            continue;
        }
        let champs: Vec<&str> = ligne.split('|').collect();
        match champs.first().copied() {
            Some("VENTE") => {
                if champs.len() != 5 {
                    return Err(format!("ligne VENTE malformee : {ligne}").into());
                }
                let montant: Decimal = champs[3].parse().map_err(|_| "montant illisible")?;
                let v = Vente {
                    reference: champs[1].to_string(),
                    date: champs[2].to_string(),
                    total_ttc: montant,
                };
                chaine = empreinte(&chaine, &v);
                total += v.total_ttc;
                ventes += 1;
                if chaine != champs[4] {
                    return Ok(VerificationArchive {
                        date: String::new(),
                        ventes,
                        total,
                        intact: false,
                    });
                }
            }
            Some("SCELLE") => {
                if champs.len() != 5 {
                    return Err(format!("ligne SCELLE malformee : {ligne}").into());
                }
                sceau = Some((
                    champs[1].to_string(),
                    champs[2].to_string(),
                    champs[3].parse().unwrap_or(0),
                    champs[4].parse().unwrap_or_default(),
                ));
            }
            Some(autre) => return Err(format!("ligne inconnue : {autre}").into()),
            None => return Err("ligne vide".into()),
        }
    }

    let (date, empreinte_finale, nombre, total_annonce) =
        sceau.ok_or("archive sans ligne SCELLE")?;

    Ok(VerificationArchive {
        intact: empreinte_finale == chaine && nombre == ventes && total_annonce == total,
        date,
        ventes,
        total,
    })
}

impl Scelle {
    /// Le scellé correspond-il à ce qui est enregistré ?
    ///
    /// `None` quand aucun scellé n'a encore été posé : on ne peut pas
    /// comparer, et ce n'est pas une falsification.
    pub fn intact(&self) -> Option<bool> {
        self.empreinte_enregistree
            .as_ref()
            .map(|enr| enr == &self.empreinte)
    }
}

/// Une ligne de vente, telle qu'elle entre dans la chaîne.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Vente {
    reference: String,
    total_ttc: Decimal,
    date: String,
}

/// Empreinte d'une vente, chaînée à la précédente.
///
/// L'ordre des champs est fixe et documenté : si on le changeait, tous les
/// scellés déjà posés deviendraient faux. C'est voulu — un scellé qui survit
/// à un changement de format ne prouve rien.
fn empreinte(precedente: &str, v: &Vente) -> String {
    let mut h = Sha256::new();
    h.update(precedente.as_bytes());
    h.update(b"|");
    h.update(v.reference.as_bytes());
    h.update(b"|");
    h.update(v.date.as_bytes());
    h.update(b"|");
    h.update(v.total_ttc.to_string().as_bytes());
    format!("{:x}", h.finalize())
}

/// Nom de la constante qui porte le scellé d'une journée.
fn nom_constante(date: &str) -> String {
    format!("DOLIBARR_RS_SCELLE_{date}")
}

/// Lit les ventes de caisse d'une journée, dans un ordre stable.
///
/// L'ordre est `ref_ext`, pas `rowid` : un `rowid` peut être réattribué après
/// suppression, la référence ne peut pas.
fn ventes_du_jour(pool: &Pool, entity: i64, date: &str) -> Result<Vec<Vente>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let motif = format!("POS-{}-%", date.replace('-', ""));
    let lignes: Vec<(Option<String>, Option<String>, Option<String>)> = conn.exec(
        "SELECT COALESCE(ref_ext, ''), DATE_FORMAT(datef, '%Y-%m-%d'), \
                CAST(total_ttc AS CHAR) \
         FROM llx_facture \
         WHERE entity = :entity AND ref_ext LIKE :motif \
         ORDER BY ref_ext ASC",
        params! { "entity" => entity, "motif" => motif },
    )?;

    Ok(lignes
        .into_iter()
        .filter_map(|(reference, jour, total)| {
            let reference = reference?;
            if reference.is_empty() {
                return None;
            }
            Some(Vente {
                reference,
                total_ttc: total
                    .and_then(|t| t.parse::<Decimal>().ok())
                    .unwrap_or_default(),
                date: jour.unwrap_or_else(|| date.to_string()),
            })
        })
        .collect())
}

/// Calcule le scellé d'une journée sans rien écrire.
pub fn calculer(pool: &Pool, entity: i64, date: &str) -> Result<Scelle, Box<dyn Error>> {
    let ventes = ventes_du_jour(pool, entity, date)?;
    let mut chaine = String::from("GENESE");
    let mut total = Decimal::ZERO;
    for v in &ventes {
        chaine = empreinte(&chaine, v);
        total += v.total_ttc;
    }

    Ok(Scelle {
        date: date.to_string(),
        ventes: ventes.len(),
        total,
        empreinte: chaine,
        empreinte_enregistree: lire_scelle(pool, entity, date)?,
    })
}

/// Pose le scellé du jour, ou le vérifie s'il existe déjà.
///
/// Un scellé déjà posé **n'est jamais écrasé** : sinon, falsifier puis resceller
/// suffirait. Si le calcul ne correspond plus, la fonction rend le désaccord au
/// lieu de réparer.
pub fn sceller(pool: &Pool, entity: i64, date: &str) -> Result<Scelle, Box<dyn Error>> {
    let scelle = calculer(pool, entity, date)?;
    if scelle.empreinte_enregistree.is_some() {
        return Ok(scelle);
    }
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_const (name, entity, value, type, visible, note) \
         VALUES (:nom, :entity, :valeur, 'string', 0, :note) \
         ON DUPLICATE KEY UPDATE value = VALUES(value)",
        params! {
            "nom" => nom_constante(date),
            "entity" => entity,
            "valeur" => scelle.empreinte.as_str(),
            "note" => format!("scelle caisse {} : {} ventes", date, scelle.ventes),
        },
    )?;
    Ok(scelle)
}

/// Le scellé enregistré pour cette journée, s'il existe.
pub fn lire_scelle(pool: &Pool, entity: i64, date: &str) -> Result<Option<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let v: Option<String> = conn.exec_first(
        "SELECT value FROM llx_const WHERE name = :nom AND entity = :entity LIMIT 1",
        params! { "nom" => nom_constante(date), "entity" => entity },
    )?;
    Ok(v.filter(|s| !s.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn vente(reference: &str, total: &str) -> Vente {
        Vente {
            reference: reference.to_string(),
            total_ttc: Decimal::from_str(total).unwrap(),
            date: "2026-11-04".to_string(),
        }
    }

    #[test]
    fn la_chaine_depend_de_l_ordre_et_du_montant() {
        let a = vente("POS-20261104-1", "10.00");
        let b = vente("POS-20261104-2", "20.00");

        let c1 = empreinte(&empreinte("GENESE", &a), &b);
        let c2 = empreinte(&empreinte("GENESE", &b), &a);
        assert_ne!(c1, c2, "l'ordre doit changer l'empreinte");

        let a_modifie = vente("POS-20261104-1", "10.01");
        let c3 = empreinte(&empreinte("GENESE", &a_modifie), &b);
        assert_ne!(c1, c3, "un centime doit changer l'empreinte");
    }

    #[test]
    fn une_vente_en_moins_change_l_empreinte() {
        let a = vente("POS-20261104-1", "10.00");
        let b = vente("POS-20261104-2", "20.00");
        let complete = empreinte(&empreinte("GENESE", &a), &b);
        let amputee = empreinte("GENESE", &a);
        assert_ne!(complete, amputee);
    }

    #[test]
    fn un_scelle_non_pose_n_est_pas_un_desaccord() {
        let s = Scelle {
            date: "2026-11-04".into(),
            ventes: 0,
            total: Decimal::ZERO,
            empreinte: "abc".into(),
            empreinte_enregistree: None,
        };
        assert_eq!(s.intact(), None);
    }

    #[test]
    fn un_scelle_different_est_un_desaccord() {
        let s = Scelle {
            date: "2026-11-04".into(),
            ventes: 1,
            total: Decimal::ONE,
            empreinte: "abc".into(),
            empreinte_enregistree: Some("def".into()),
        };
        assert_eq!(s.intact(), Some(false));
    }
}
