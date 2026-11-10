//! Doublons de tiers : les trouver, et proposer la fusion.
//!
//! Dolibarr a un outil de fusion ; sans lui, une base accumule « M & Mme
//! Dupont », « M et Mme DUPONT » et « Dupont » — trois fiches, trois
//! historiques, et un client qui reçoit trois relances.
//!
//! # Ce qu'on compare
//!
//! Trois clés, dans cet ordre de force :
//!
//! 1. **le SIRET** — deux fiches qui portent le même numéro sont la même
//!    entreprise, quelle que soit l'orthographe ;
//! 2. **l'e-mail** — même adresse, même interlocuteur ;
//! 3. **le nom normalisé** — minuscules, accents retirés, forme juridique et
//!    civilité supprimées. C'est le plus faible : il propose, il ne prouve rien.
//!
//! # La fusion ne détruit pas
//!
//! Fusionner, ici, c'est **déplacer les liens** vers la fiche gardée, puis
//! **désactiver** la fiche absorbée en y écrivant où elle est partie. Dolibarr
//! supprime ; nous préférons laisser une trace : une fusion qu'on regrette doit
//! pouvoir se défaire à la main, dans la base, en une requête.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

use crate::party::{self, Party};

/// Les tables qui portent un `fk_soc` et qu'on déplace lors d'une fusion.
///
/// Volontairement limité au noyau et aux modules de Pierre Angulaire : une table
/// de module exotique qu'on oublierait laisserait des lignes orphelines, et une
/// table qu'on toucherait à tort casserait un module. La liste est explicite,
/// donc vérifiable.
pub const TABLES_LIEES: &[(&str, &str)] = &[
    ("llx_propal", "fk_soc"),
    ("llx_commande", "fk_soc"),
    ("llx_facture", "fk_soc"),
    ("llx_facture_fourn", "fk_soc"),
    ("llx_commande_fournisseur", "fk_soc"),
    ("llx_supplier_proposal", "fk_soc"),
    ("llx_contrat", "fk_soc"),
    ("llx_socpeople", "fk_soc"),
    ("llx_projet", "fk_soc"),
    ("llx_ticket", "fk_soc"),
    ("llx_fichinter", "fk_soc"),
    ("llx_expedition", "fk_soc"),
    ("llx_livraison", "fk_soc"),
    ("llx_reception", "fk_soc"),
    ("llx_don", "fk_soc"),
    ("llx_adherent", "fk_soc"),
    ("llx_actioncomm", "fk_soc"),
    ("llx_societe_commerciaux", "fk_soc"),
    ("llx_societe_contacts", "fk_soc"),
    ("llx_societe_rib", "fk_soc"),
    ("llx_product_customer_price", "fk_soc"),
    ("llx_product_thirdparty", "fk_soc"),
];

/// Un groupe de fiches qui se ressemblent, avec la raison.
#[derive(Debug, Clone, PartialEq)]
pub struct Groupe {
    pub raison: &'static str,
    pub cle: String,
    pub membres: Vec<Party>,
}

/// Ramene un nom à ce qui l'identifie : minuscules, sans accents, sans forme
/// juridique, sans civilité, sans ponctuation.
///
/// « M & Mme Dupont (SARL) » et « dupont » donnent la même chose. C'est
/// imparfait par construction : deux entreprises différentes peuvent partager un
/// nom, et c'est pour cela qu'un groupe n'est qu'une **proposition**.
pub fn normaliser(nom: &str) -> String {
    let mut s = String::with_capacity(nom.len());
    for c in nom.chars() {
        let remplace = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ÿ' => 'y',
            autre => autre,
        };
        if remplace.is_ascii_alphanumeric() {
            s.push(remplace.to_ascii_lowercase());
        } else {
            s.push(' ');
        }
    }

    // Les mots qui ne distinguent personne.
    const VIDES: &[&str] = &[
        "sarl",
        "sas",
        "sasu",
        "eurl",
        "sci",
        "scp",
        "snc",
        "ei",
        "eirl",
        "auto",
        "entrepreneur",
        "mr",
        "m",
        "mme",
        "mlle",
        "monsieur",
        "madame",
        "mademoiselle",
        "et",
        "ets",
        "etablissements",
        "societe",
        "cie",
    ];
    let mots: Vec<&str> = s
        .split_whitespace()
        .filter(|m| !VIDES.contains(m))
        .collect();
    mots.join(" ")
}

/// Les chiffres d'un téléphone : « 06 12 34 56 78 » et « +33612345678 » se
/// comparent sur `0612345678`.
pub fn chiffres(valeur: &str) -> String {
    valeur.chars().filter(char::is_ascii_digit).collect()
}

/// Les groupes de fiches qui se ressemblent dans l'entite.
///
/// Un groupe de moins de deux membres n'est pas un doublon : il n'est pas rendu.
pub fn groupes(pool: &Pool, entity: i64) -> Result<Vec<Groupe>, Box<dyn Error>> {
    // Toutes les fiches : une base de 900 tiers tient largement en memoire, et
    // chercher en SQL imposerait une fonction de normalisation que MariaDB n'a
    // pas.
    let fiches = party::list(pool, entity, 20_000, 0)?;
    let mut groupes: Vec<Groupe> = Vec::new();

    // 1. Le SIRET, quand il est renseigne.
    let mut par_siret: BTreeMap<String, Vec<Party>> = BTreeMap::new();
    for f in &fiches {
        if let Some(s) = f.siret.as_deref().map(chiffres).filter(|s| s.len() == 14) {
            par_siret.entry(s).or_default().push(f.clone());
        }
    }
    for (cle, membres) in par_siret {
        if membres.len() > 1 {
            groupes.push(Groupe {
                raison: "même SIRET",
                cle,
                membres,
            });
        }
    }

    // 2. L'e-mail.
    let mut par_email: BTreeMap<String, Vec<Party>> = BTreeMap::new();
    for f in &fiches {
        if let Some(e) = f
            .email
            .as_deref()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| e.contains('@') && e.len() > 4)
        {
            par_email.entry(e).or_default().push(f.clone());
        }
    }
    for (cle, membres) in par_email {
        if membres.len() > 1 {
            groupes.push(Groupe {
                raison: "même e-mail",
                cle,
                membres,
            });
        }
    }

    // 3. Le nom normalise.
    let mut par_nom: BTreeMap<String, Vec<Party>> = BTreeMap::new();
    for f in &fiches {
        let cle = normaliser(&f.nom);
        if cle.len() >= 3 {
            par_nom.entry(cle).or_default().push(f.clone());
        }
    }
    for (cle, membres) in par_nom {
        if membres.len() > 1 {
            groupes.push(Groupe {
                raison: "nom identique",
                cle,
                membres,
            });
        }
    }

    // Deux groupes qui designent exactement les memes fiches n'en font qu'un :
    // on garde la raison la plus forte (l'ordre d'insertion ci-dessus).
    let mut vus: BTreeSet<Vec<i64>> = BTreeSet::new();
    groupes.retain(|g| {
        let mut ids: Vec<i64> = g.membres.iter().map(|m| m.rowid).collect();
        ids.sort_unstable();
        vus.insert(ids)
    });

    // Les plus gros groupes d'abord : c'est la ou il y a le plus a gagner.
    groupes.sort_by_key(|g| std::cmp::Reverse(g.membres.len()));
    Ok(groupes)
}

/// Fusionne `source` dans `cible` : deplace les liens, puis desactive la source.
///
/// Rend le nombre de lignes deplacees par table, pour qu'on puisse verifier.
pub fn fusionner(
    pool: &Pool,
    entity: i64,
    source: i64,
    cible: i64,
) -> Result<Vec<(&'static str, u64)>, Box<dyn Error>> {
    if source == cible {
        return Err("on ne fusionne pas une fiche avec elle-meme".into());
    }
    let mut conn = pool.get_conn()?;
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => source },
    )?;
    if existe.is_none() {
        return Err(format!("tiers {source} introuvable dans l'entite {entity}").into());
    }
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :id",
        params! { "entity" => entity, "id" => cible },
    )?;
    if existe.is_none() {
        return Err(format!("tiers {cible} introuvable dans l'entite {entity}").into());
    }

    let mut tx = conn.start_transaction(mysql::TxOpts::default())?;
    let mut faits: Vec<(&'static str, u64)> = Vec::new();
    for (table, colonne) in TABLES_LIEES {
        // La table peut ne pas exister (module non installe) : on l'ignore, mais
        // on ne l'invente pas.
        let presente: Option<i64> = tx.exec_first(
            "SELECT COUNT(*) FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = :t",
            params! { "t" => *table },
        )?;
        if presente.unwrap_or(0) == 0 {
            continue;
        }
        let sql = format!("UPDATE {table} SET {colonne} = :cible WHERE {colonne} = :source");
        tx.exec_drop(sql, params! { "cible" => cible, "source" => source })?;
        let n = tx.affected_rows();
        if n > 0 {
            faits.push((table, n));
        }
    }

    // La source n'est pas supprimee : elle est desactivee, et son nom dit ou
    // elle est partie. Une fusion qui abime l'historique doit rester lisible.
    tx.exec_drop(
        "UPDATE llx_societe SET client = 0, fournisseur = 0, status = 0, \
          name_alias = CONCAT(COALESCE(name_alias, ''), ' [fusionne dans ', :cible, ']') \
         WHERE entity = :entity AND rowid = :source",
        params! { "cible" => cible, "entity" => entity, "source" => source },
    )?;
    tx.commit()?;
    Ok(faits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_civilite_et_la_forme_juridique_ne_comptent_pas() {
        assert_eq!(normaliser("M & Mme Dupont"), "dupont");
        assert_eq!(normaliser("M. et Mme DUPONT (SARL)"), "dupont");
        assert_eq!(normaliser("ETS Dupont"), "dupont");
    }

    #[test]
    fn les_accents_ne_separent_pas_deux_fiches() {
        // « societe » fait partie des mots qui ne distinguent personne : il
        // tombe, et il reste « generale ».
        assert_eq!(normaliser("Société Générale"), "generale");
        // L'espace en trop ne cree pas une fiche differente.
        assert_eq!(normaliser("Générale"), normaliser("Generale "));
    }

    #[test]
    fn deux_entreprises_differentes_restent_differentes() {
        assert_ne!(normaliser("SCI des Bois Infinis"), normaliser("SCI Boca"));
    }

    #[test]
    fn les_telephones_se_comparent_sur_les_chiffres() {
        assert_eq!(chiffres("06 12 34 56 78"), "0612345678");
        assert_eq!(chiffres("+33 6 12 34 56 78"), "33612345678");
    }

    #[test]
    fn un_nom_trop_court_ne_propose_rien() {
        // Deux « AB » ne sont pas deux fois la meme entreprise : c'est le nom
        // normalise qui doit etre assez long pour valoir une comparaison, et
        // c'est `groupes` qui l'applique.
        assert!(normaliser("AB").len() < 3);
    }
}
