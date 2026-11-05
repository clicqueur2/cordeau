//! Modeles d'e-mail : les messages pre-ecrits, par type d'objet.
//!
//! Dolibarr garde ses modeles dans `llx_c_email_templates`, avec un
//! `type_template` qui dit a quoi ils servent (`invoice_send`, `propal_send`…) et
//! deux textes : `topic` (l'objet) et `content` (le corps).
//!
//! Le format des textes est celui de Dolibarr :
//!
//! - `__VARIABLE__` : une valeur tiree de l'objet (`__REF__`, `__TOTAL_TTC__`…) ;
//! - `__(CleDeTraduction)__` : un libelle traduit du catalogue.
//!
//! On ne reimplemente pas les quelques centaines de variables du cceur. On
//! substitue **celles qu'on fournit**, on traduit les cles via le catalogue, et
//! **on laisse le reste visible** : un `__MYCOMPANY_NAME__` non remplace se voit
//! a l'ecran, il ne sort pas vide dans un mail client.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Un modele d'e-mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modele {
    pub rowid: i64,
    pub libelle: String,
    pub type_template: String,
    /// L'objet du message.
    pub sujet: String,
    /// Le corps, en HTML Dolibarr.
    pub corps: String,
    /// Le modele propose par defaut pour ce type.
    pub defaut: bool,
    /// Actif : un modele desactive ne doit pas apparaitre dans les choix.
    pub actif: bool,
}

/// Les types de modeles qui concernent la vente.
pub const TYPES_VENTE: &[(&str, &str)] = &[
    ("invoice_send", "Envoi de facture"),
    ("propal_send", "Envoi de devis"),
    ("order_send", "Envoi de commande"),
    ("shipping_send", "Envoi de bon de livraison"),
    ("facture_rec", "Facture récurrente"),
];

/// Les modeles actifs d'un type, celui par defaut en tete.
///
/// On lit l'entite courante **et** l'entite 0 : Dolibarr seme ses modeles
/// globaux, et une societe peut avoir les siens.
pub fn pour(pool: &Pool, entity: i64, type_template: &str) -> Result<Vec<Modele>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT rowid, COALESCE(label, ''), COALESCE(type_template, ''), \
                COALESCE(topic, ''), COALESCE(content, ''), \
                COALESCE(defaultfortype, 0), COALESCE(active, 0) \
         FROM llx_c_email_templates \
         WHERE (entity = :entity OR entity = 0) AND type_template = :modele_type \
           AND active = 1 \
         ORDER BY defaultfortype DESC, label ASC",
        params! { "entity" => entity, "modele_type" => type_template },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| Modele {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            libelle: r
                .get_opt::<String, _>(1)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            type_template: r
                .get_opt::<String, _>(2)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            sujet: r
                .get_opt::<String, _>(3)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            corps: r
                .get_opt::<String, _>(4)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            defaut: r.get_opt::<i32, _>(5).and_then(|v| v.ok()).unwrap_or(0) != 0,
            actif: r.get_opt::<i32, _>(6).and_then(|v| v.ok()).unwrap_or(0) != 0,
        })
        .collect())
}

/// Tous les modeles, actifs ou non, pour l'ecran d'administration.
pub fn tous(pool: &Pool, entity: i64) -> Result<Vec<Modele>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT rowid, COALESCE(label, ''), COALESCE(type_template, ''), \
                COALESCE(topic, ''), COALESCE(content, ''), \
                COALESCE(defaultfortype, 0), COALESCE(active, 0) \
         FROM llx_c_email_templates \
         WHERE entity = :entity OR entity = 0 \
         ORDER BY type_template ASC, defaultfortype DESC, label ASC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| Modele {
            rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
            libelle: r
                .get_opt::<String, _>(1)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            type_template: r
                .get_opt::<String, _>(2)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            sujet: r
                .get_opt::<String, _>(3)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            corps: r
                .get_opt::<String, _>(4)
                .and_then(|v| v.ok())
                .unwrap_or_default(),
            defaut: r.get_opt::<i32, _>(5).and_then(|v| v.ok()).unwrap_or(0) != 0,
            actif: r.get_opt::<i32, _>(6).and_then(|v| v.ok()).unwrap_or(0) != 0,
        })
        .collect())
}

/// Cree un modele pour l'entite.
pub fn creer(
    pool: &Pool,
    entity: i64,
    type_template: &str,
    libelle: &str,
    sujet: &str,
    corps: &str,
    defaut: bool,
) -> Result<i64, Box<dyn Error>> {
    if libelle.trim().is_empty() {
        return Err("le libellé est obligatoire".into());
    }
    if !TYPES_VENTE.iter().any(|(t, _)| *t == type_template) {
        return Err(format!("type de modèle inconnu : {type_template}").into());
    }
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "INSERT INTO llx_c_email_templates \
           (entity, module, type_template, lang, private, datec, label, position, \
            defaultfortype, enabled, active, topic, content, content_lines, joinfiles) \
         VALUES (:entity, 'dolibarr_rs', :modele_type, 'fr_FR', 1, NOW(), :label, 0, \
                 :defaut, '1', 1, :sujet, :corps, '', '')",
        params! {
            "entity" => entity,
            "modele_type" => type_template,
            "label" => libelle.trim(),
            "defaut" => if defaut { 1 } else { 0 },
            "sujet" => sujet,
            "corps" => corps,
        },
    )?;
    Ok(conn.last_insert_id() as i64)
}

/// Active ou desactive un modele.
pub fn basculer(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    actif: bool,
) -> Result<usize, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE llx_c_email_templates SET active = :actif \
         WHERE rowid = :id AND entity = :entity",
        params! { "actif" => if actif { 1 } else { 0 }, "id" => rowid, "entity" => entity },
    )?;
    Ok(conn.affected_rows() as usize)
}

/// Substitue les variables d'un texte.
///
/// `variables` : les paires `(nom, valeur)` que l'appelant sait remplir.
/// `traduire` : ce qui transforme une cle de traduction en libelle.
///
/// Une variable inconnue est **laissee telle quelle** : on preferera toujours
/// voir `__MYCOMPANY_NAME__` dans un apercu a un trou dans un mail envoye.
pub fn substituer(
    texte: &str,
    variables: &[(&str, String)],
    traduire: &dyn Fn(&str) -> String,
) -> String {
    let mut sortie = String::with_capacity(texte.len());
    let mut reste = texte;

    while let Some(debut) = reste.find("__") {
        sortie.push_str(&reste[..debut]);
        let apres = &reste[debut + 2..];
        let Some(fin) = apres.find("__") else {
            // Pas de fermeture : on recopie tel quel et on s'arrete.
            sortie.push_str(&reste[debut..]);
            return sortie;
        };
        let cle = &apres[..fin];
        let consomme = 2 + fin + 2;

        if let Some((_, valeur)) = variables.iter().find(|(nom, _)| *nom == cle) {
            sortie.push_str(valeur);
        } else if cle.starts_with('(') && cle.ends_with(')') {
            // `__(CleDeTraduction)__` : on passe par le catalogue.
            let interne = &cle[1..cle.len() - 1];
            let traduit = traduire(interne);
            if traduit == interne {
                // Cle absente du catalogue : on garde la forme d'origine, elle
                // se voit et ne casse rien.
                sortie.push_str("__");
                sortie.push_str(cle);
                sortie.push_str("__");
            } else {
                sortie.push_str(&traduit);
            }
        } else {
            // Variable que l'appelant ne fournit pas : on la laisse visible.
            sortie.push_str("__");
            sortie.push_str(cle);
            sortie.push_str("__");
        }
        reste = &reste[debut + consomme..];
    }
    sortie.push_str(reste);
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sans_traduction(cle: &str) -> String {
        cle.to_string()
    }

    #[test]
    fn une_variable_connue_est_remplacee() {
        let v = vec![("REF", "F-2601-0001".to_string())];
        assert_eq!(
            substituer("Votre facture __REF__", &v, &sans_traduction),
            "Votre facture F-2601-0001"
        );
    }

    #[test]
    fn plusieurs_variables_se_remplacent() {
        let v = vec![
            ("REF", "F-1".to_string()),
            ("TOTAL", "120,00 €".to_string()),
        ];
        assert_eq!(
            substituer("__REF__ pour __TOTAL__", &v, &sans_traduction),
            "F-1 pour 120,00 €"
        );
    }

    /// Une variable que l'appelant ne fournit pas reste visible.
    #[test]
    fn une_variable_inconnue_reste_visible() {
        let v = vec![("REF", "F-1".to_string())];
        assert_eq!(
            substituer("__REF__ __MYCOMPANY_NAME__", &v, &sans_traduction),
            "F-1 __MYCOMPANY_NAME__"
        );
    }

    #[test]
    fn une_cle_de_traduction_passe_par_le_catalogue() {
        let traduire = |cle: &str| match cle {
            "Hello" => "Bonjour".to_string(),
            autre => autre.to_string(),
        };
        assert_eq!(
            substituer("__(Hello)__, merci", &[], &traduire),
            "Bonjour, merci"
        );
    }

    /// Une cle absente du catalogue garde sa forme : elle se voit.
    #[test]
    fn une_cle_absente_du_catalogue_reste_telle_quelle() {
        assert_eq!(
            substituer("__(Inconnue)__", &[], &sans_traduction),
            "__(Inconnue)__"
        );
    }

    #[test]
    fn un_texte_sans_variable_est_rendu_intact() {
        assert_eq!(substituer("Bonjour", &[], &sans_traduction), "Bonjour");
        assert_eq!(substituer("", &[], &sans_traduction), "");
    }

    /// Un `__` non ferme ne doit pas faire perdre la fin du texte.
    #[test]
    fn un_marqueur_non_ferme_est_recopie() {
        assert_eq!(
            substituer("Total __REF", &[], &sans_traduction),
            "Total __REF"
        );
    }

    #[test]
    fn les_types_de_vente_sont_ceux_de_dolibarr() {
        for (t, _) in TYPES_VENTE {
            assert!(!t.trim().is_empty());
            assert!(t.contains('_') || *t == "invoice_send", "{t}");
        }
    }
}
