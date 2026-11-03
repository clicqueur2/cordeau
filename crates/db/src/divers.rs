//! Lectures transverses : reglements, contrats, listes filtrees de tiers.
//!
//! Ces ecrans n'ont pas de logique propre : ils montrent ce que Dolibarr a
//! deja en base, sans le recalculer.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, Value};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reglement {
    pub rowid: i64,
    pub ref_: String,
    pub date: Option<String>,
    pub montant: Decimal,
    pub mode: Option<String>,
    pub tiers: Option<String>,
    pub factures: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contrat {
    pub rowid: i64,
    pub ref_: String,
    pub tiers: Option<String>,
    pub date: Option<String>,
    pub statut: i16,
    pub total_ttc: Option<Decimal>,
}

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// Reglements enregistres, du plus recent au plus ancien, avec le tiers et les
/// factures qu'ils lettrent.
pub fn reglements(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Reglement>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT p.rowid, p.ref, DATE_FORMAT(p.datep, '%Y-%m-%d'), \
                CAST(p.amount AS CHAR), t.code AS mode, s.nom, \
                GROUP_CONCAT(DISTINCT f.ref ORDER BY f.ref SEPARATOR ', ') \
         FROM llx_paiement p \
         LEFT JOIN llx_c_paiement t ON t.id = p.fk_paiement \
         LEFT JOIN llx_paiement_facture pf ON pf.fk_paiement = p.rowid \
         LEFT JOIN llx_facture f ON f.rowid = pf.fk_facture \
         LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE p.entity = :entity \
         GROUP BY p.rowid, p.ref, p.datep, p.amount, t.code, s.nom \
         ORDER BY p.datep DESC, p.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Reglement {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            date: opt_s(&r, 2),
            montant: opt_dec(&r, 3).unwrap_or_default(),
            mode: opt_s(&r, 4),
            tiers: opt_s(&r, 5),
            factures: opt_s(&r, 6),
        })
        .collect())
}

/// Contrats clients (`llx_contrat`).
pub fn contrats(pool: &Pool, entity: i64, limit: u32) -> Result<Vec<Contrat>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT c.rowid, c.ref, s.nom, DATE_FORMAT(c.date_contrat, '%Y-%m-%d'), \
                c.statut, CAST(c.total_ttc AS CHAR) \
         FROM llx_contrat c LEFT JOIN llx_societe s ON s.rowid = c.fk_soc \
         WHERE c.entity = :entity \
         ORDER BY c.date_contrat DESC, c.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Contrat {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            tiers: opt_s(&r, 2),
            date: opt_s(&r, 3),
            statut: opt_i(&r, 4).unwrap_or(0) as i16,
            total_ttc: opt_dec(&r, 5),
        })
        .collect())
}

/// Tiers filtres : `prospect` (`client = 2` ou `3`) ou `fournisseur`.
///
/// Dolibarr encode le role dans deux colonnes : `client` (0 aucun, 1 client,
/// 2 prospect, 3 client et prospect) et `fournisseur` (0/1).
pub fn tiers_filtres(
    pool: &Pool,
    entity: i64,
    filtre: &str,
    limit: u32,
) -> Result<Vec<crate::party::Party>, Box<dyn Error>> {
    let condition = match filtre {
        "prospect" => "client IN (2, 3)",
        "fournisseur" => "fournisseur = 1",
        "client" => "client IN (1, 3)",
        _ => "1 = 1",
    };
    crate::party::list_filtre(pool, entity, condition, limit)
}

/// Une expedition (`llx_expedition`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Expedition {
    pub rowid: i64,
    pub ref_: String,
    pub tiers: Option<String>,
    pub date: Option<String>,
    pub livraison: Option<String>,
    pub statut: i16,
}

/// Expeditions, de la plus recente a la plus ancienne.
pub fn expeditions(
    pool: &Pool,
    entity: i64,
    limit: u32,
) -> Result<Vec<Expedition>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT e.rowid, e.ref, s.nom, DATE_FORMAT(e.date_creation, '%Y-%m-%d'), \
                DATE_FORMAT(e.date_delivery, '%Y-%m-%d'), e.fk_statut \
         FROM llx_expedition e LEFT JOIN llx_societe s ON s.rowid = e.fk_soc \
         WHERE e.entity = :entity \
         ORDER BY e.date_creation DESC, e.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| Expedition {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            tiers: opt_s(&r, 2),
            date: opt_s(&r, 3),
            livraison: opt_s(&r, 4),
            statut: opt_i(&r, 5).unwrap_or(0) as i16,
        })
        .collect())
}
/// Une commande fournisseur, en lecture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandeFourn {
    pub rowid: i64,
    pub ref_: String,
    pub ref_supplier: Option<String>,
    pub tiers: Option<String>,
    pub date: Option<String>,
    pub statut: i16,
    pub total_ttc: Option<Decimal>,
}

/// Commandes fournisseurs (llx_commande_fournisseur).
pub fn commandes_fournisseurs(
    pool: &Pool,
    entity: i64,
    limit: u32,
) -> Result<Vec<CommandeFourn>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT c.rowid, c.ref, c.ref_supplier, s.nom, \
                DATE_FORMAT(c.date_commande, '%Y-%m-%d'), c.fk_statut, \
                CAST(c.total_ttc AS CHAR) \
         FROM llx_commande_fournisseur c LEFT JOIN llx_societe s ON s.rowid = c.fk_soc \
         WHERE c.entity = :entity \
         ORDER BY c.date_commande DESC, c.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| CommandeFourn {
            rowid: opt_i(&r, 0).unwrap_or(0),
            ref_: opt_s(&r, 1).unwrap_or_default(),
            ref_supplier: opt_s(&r, 2),
            tiers: opt_s(&r, 3),
            date: opt_s(&r, 4),
            statut: opt_i(&r, 5).unwrap_or(0) as i16,
            total_ttc: opt_dec(&r, 6),
        })
        .collect())
}

/// Reference, date, montant, code du mode de paiement.
pub type PaiementFourn = (String, Option<String>, Decimal, Option<String>);

/// Paiements fournisseurs (`llx_paiementfourn`).
pub fn paiements_fournisseurs(
    pool: &Pool,
    entity: i64,
    limit: u32,
) -> Result<Vec<PaiementFourn>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT p.ref, DATE_FORMAT(p.datep, '%Y-%m-%d'), CAST(p.amount AS CHAR), \
                t.code \
         FROM llx_paiementfourn p LEFT JOIN llx_c_paiement t ON t.id = p.fk_paiement \
         WHERE p.entity = :entity ORDER BY p.datep DESC, p.rowid DESC LIMIT :limit",
        params! { "entity" => entity, "limit" => limit },
    )?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                opt_s(&r, 0).unwrap_or_default(),
                opt_s(&r, 1),
                opt_dec(&r, 2).unwrap_or_default(),
                opt_s(&r, 3),
            )
        })
        .collect())
}
/// Execute une requete de liste et renvoie ses lignes en texte.
///
/// Le SQL vient d'une table fermee cote web (aucune saisie utilisateur), et
/// :entity est toujours fourni : une liste ne peut pas deborder d'entite.
pub fn lignes_brutes(
    pool: &Pool,
    entity: i64,
    sql: &str,
) -> Result<Vec<Vec<Option<String>>>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity })?;
    Ok(rows
        .iter()
        .map(|r| {
            (0..r.len())
                .map(|i| {
                    r.get_opt::<String, _>(i).and_then(|v| v.ok()).or_else(|| {
                        r.get_opt::<i64, _>(i)
                            .and_then(|v| v.ok())
                            .map(|n| n.to_string())
                    })
                })
                .collect()
        })
        .collect())
}
/// Un identifiant SQL acceptable : lettres, chiffres, souligne.
fn identifiant_sql(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Les seules expressions SQL autorisees a etre posees d'office.
// `'phy'` : la personne morale ou physique d'un adherent, en dur comme dans
// Dolibarr pour une adhesion de particulier.
const EXPRESSIONS_SQL: &[&str] = &["NOW()", "CURDATE()", "0", "1", "'phy'"];

/// L'expression SQL est-elle dans la liste fermee ? Sert aux modules
/// declaratifs, qui n'ont pas acces a la constante privee.
pub fn expression_autorisee(expr: &str) -> bool {
    EXPRESSIONS_SQL.contains(&expr)
}

/// Un identifiant SQL acceptable ? Meme regle que partout ici : lettres,
/// chiffres, souligne.
pub fn identifiant_autorise(s: &str) -> bool {
    identifiant_sql(s)
}

/// Les expressions acceptees, pour les messages d'erreur.
pub fn expressions_autorisees() -> &'static [&'static str] {
    EXPRESSIONS_SQL
}

/// Une valeur de colonne, en texte, pour l'affichage et la reprise en
/// formulaire.
fn valeur_texte(r: &Row, i: usize) -> Option<String> {
    r.get_opt::<String, _>(i)
        .and_then(|v| v.ok())
        .or_else(|| {
            r.get_opt::<i64, _>(i)
                .and_then(|v| v.ok())
                .map(|n| n.to_string())
        })
        // Les colonnes `double(24,8)` ne se decodent ni en texte ni en entier :
        // sans ce troisieme essai, un montant reviendrait vide dans le formulaire.
        .or_else(|| {
            use rust_decimal::prelude::FromPrimitive;
            r.get_opt::<f64, _>(i).and_then(|v| v.ok()).map(|n| {
                Decimal::from_f64(n)
                    .map(|d| d.normalize().to_string())
                    .unwrap_or_else(|| n.to_string())
            })
        })
}

/// Insertion generique, pour les modules sans ecran d'ecriture dedie.
///
/// Le nom de table et les colonnes viennent du registre cote web (constantes),
/// mais on les valide quand meme : aucune chaine venue d'une requete HTTP ne
/// doit pouvoir atterrir dans le SQL. Les valeurs, elles, sont toutes liees.
pub fn inserer(
    pool: &Pool,
    table: &str,
    entity: i64,
    valeurs: &[(String, Option<String>)],
    exprs: &[(&'static str, &'static str)],
) -> Result<u64, Box<dyn Error>> {
    if !identifiant_sql(table) {
        return Err(format!("nom de table refuse : {table}").into());
    }

    let mut colonnes: Vec<&str> = vec!["entity"];
    let mut marques: Vec<&str> = vec!["?"];
    let mut params: Vec<Value> = vec![Value::Int(entity)];

    for (colonne, v) in valeurs {
        if !identifiant_sql(colonne) {
            return Err(format!("nom de colonne refuse : {colonne}").into());
        }
        colonnes.push(colonne);
        marques.push("?");
        params.push(match v {
            Some(s) => Value::from(s.clone()),
            None => Value::NULL,
        });
    }

    // Expressions posees d'office : uniquement des formes fermees, jamais du
    // texte libre.
    for (colonne, expr) in exprs {
        if !identifiant_sql(colonne) {
            return Err(format!("nom de colonne refuse : {colonne}").into());
        }
        if !EXPRESSIONS_SQL.contains(expr) {
            return Err(format!("expression SQL refusee : {expr}").into());
        }
        colonnes.push(colonne);
        marques.push(expr);
    }

    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({})",
        colonnes.join(", "),
        marques.join(", ")
    );
    let mut conn = pool.get_conn()?;
    conn.exec_drop(sql, params)?;
    Ok(conn.last_insert_id())
}

/// Produits d'une entite, sous la forme (identifiant, « ref - libelle »).
/// Sert aux listes deroulantes des formulaires de saisie.
pub fn produits_pour_liste(
    pool: &Pool,
    entity: i64,
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<(i64, String, Option<String>)> = conn.exec(
        "SELECT rowid, ref, label FROM llx_product WHERE entity = :entity \
         ORDER BY ref ASC LIMIT 500",
        params! { "entity" => entity },
    )?;
    Ok(rows
        .into_iter()
        .map(|(id, r, l)| {
            let libelle = match l {
                Some(l) if !l.trim().is_empty() => format!("{r} - {l}"),
                _ => r,
            };
            (id.to_string(), libelle)
        })
        .collect())
}

/// Lit une ligne d'une table, colonne par colonne, en texte.
///
/// `table` et `colonnes` viennent du registre cote web ; ils sont valides
/// comme partout ailleurs, et l'entite est toujours dans le WHERE.
pub fn lire_ligne_par(
    pool: &Pool,
    table: &str,
    cle: &str,
    entity: i64,
    id: i64,
    colonnes: &[&str],
) -> Result<Option<Vec<Option<String>>>, Box<dyn Error>> {
    if !identifiant_sql(table)
        || !identifiant_sql(cle)
        || colonnes.iter().any(|c| !identifiant_sql(c))
    {
        return Err("nom de table ou de colonne refuse".into());
    }
    // Tout est caste en texte : une DATE, un double(24,8) et un entier se
    // relisent alors de la meme facon, sans deviner le type a l'arrivee.
    let liste = colonnes
        .iter()
        .map(|c| format!("CAST({c} AS CHAR)"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT {liste} FROM {table} WHERE {cle} = ? AND entity = ?");
    let mut conn = pool.get_conn()?;
    let row: Option<Row> = conn.exec_first(sql, (id, entity))?;
    Ok(row.map(|r| {
        (0..r.len())
            .map(|i| valeur_texte(&r, i))
            .collect::<Vec<_>>()
    }))
}

/// Met a jour une ligne. Renvoie le nombre de lignes touchees (0 = rien).
pub fn mettre_a_jour_par(
    pool: &Pool,
    table: &str,
    cle: &str,
    entity: i64,
    id: i64,
    valeurs: &[(String, Option<String>)],
    exprs: &[(&'static str, &'static str)],
) -> Result<u64, Box<dyn Error>> {
    if !identifiant_sql(table) || !identifiant_sql(cle) {
        return Err(format!("nom de table refuse : {table}").into());
    }
    let mut morceaux: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();

    for (colonne, v) in valeurs {
        if !identifiant_sql(colonne) {
            return Err(format!("nom de colonne refuse : {colonne}").into());
        }
        morceaux.push(format!("{colonne} = ?"));
        params.push(match v {
            Some(s) => Value::from(s.clone()),
            None => Value::NULL,
        });
    }
    for (colonne, expr) in exprs {
        if !identifiant_sql(colonne) || !EXPRESSIONS_SQL.contains(expr) {
            return Err(format!("expression refusee sur {colonne} : {expr}").into());
        }
        morceaux.push(format!("{colonne} = {expr}"));
    }
    if morceaux.is_empty() {
        return Ok(0);
    }
    params.push(Value::Int(id));
    params.push(Value::Int(entity));

    let sql = format!(
        "UPDATE {table} SET {} WHERE {cle} = ? AND entity = ?",
        morceaux.join(", ")
    );
    let mut conn = pool.get_conn()?;
    conn.exec_drop(sql, params)?;
    Ok(conn.affected_rows())
}

/// Supprime une ligne. Renvoie le nombre de lignes parties (0 = rien).
pub fn supprimer_ligne_par(
    pool: &Pool,
    table: &str,
    cle: &str,
    entity: i64,
    id: i64,
) -> Result<u64, Box<dyn Error>> {
    if !identifiant_sql(table) || !identifiant_sql(cle) {
        return Err(format!("nom de table refuse : {table}").into());
    }
    let sql = format!("DELETE FROM {table} WHERE {cle} = ? AND entity = ?");
    let mut conn = pool.get_conn()?;
    conn.exec_drop(sql, (id, entity))?;
    Ok(conn.affected_rows())
}

/// Options d'une liste deroulante, par source nommee.
///
/// Les sources sont fermees : une source inconnue rend une liste vide plutot
/// que d'aller chercher une table au hasard.
pub fn options_liste(
    pool: &Pool,
    entity: i64,
    source: &str,
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    match source {
        "produits" => produits_pour_liste(pool, entity),
        "types_adherent" => {
            let mut conn = pool.get_conn()?;
            let rows: Vec<(i64, String)> = conn.exec(
                "SELECT rowid, libelle FROM llx_adherent_type WHERE entity = :entity \
                 ORDER BY rowid ASC",
                params! { "entity" => entity },
            )?;
            Ok(rows.into_iter().map(|(i, l)| (i.to_string(), l)).collect())
        }
        "tiers" => {
            let mut conn = pool.get_conn()?;
            let rows: Vec<(i64, String)> = conn.exec(
                "SELECT rowid, nom FROM llx_societe WHERE entity = :entity \
                 ORDER BY nom ASC LIMIT 500",
                params! { "entity" => entity },
            )?;
            Ok(rows.into_iter().map(|(i, l)| (i.to_string(), l)).collect())
        }
        // Types de categorie, tels que Dolibarr les numerote.
        "types_categorie" => Ok(vec![
            ("0".to_string(), "Produit ou service".to_string()),
            ("1".to_string(), "Tiers".to_string()),
            ("2".to_string(), "Contact".to_string()),
            ("3".to_string(), "Adherent".to_string()),
            ("4".to_string(), "Projet".to_string()),
        ]),
        _ => Ok(Vec::new()),
    }
}

/// Numero suivant d'une reference : `prefixe` + compteur sur 4 chiffres.
///
/// Sert aux tables dont Dolibarr impose une reference non nulle et dont le
/// module de numerotation n'est pas encore porte.
pub fn suivant_ref(
    pool: &Pool,
    table: &str,
    colonne: &str,
    prefixe: &str,
    entity: i64,
) -> Result<String, Box<dyn Error>> {
    if !identifiant_sql(table) || !identifiant_sql(colonne) {
        return Err("nom de table ou de colonne refuse".into());
    }
    let debut = prefixe.chars().count() + 1;
    let sql = format!(
        "SELECT MAX(CAST(SUBSTRING({colonne}, {debut}) AS UNSIGNED)) FROM {table} \
         WHERE entity = ? AND {colonne} LIKE ?"
    );
    let mut conn = pool.get_conn()?;
    let max: Option<Option<u64>> = conn.exec_first(sql, (entity, format!("{prefixe}%")))?;
    let suivant = max.flatten().unwrap_or(0) + 1;
    Ok(format!("{prefixe}{suivant:04}"))
}
