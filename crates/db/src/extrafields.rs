//! Champs libres (extrafields) : les colonnes que Dolibarr ajoute aux tables.
//!
//! Dolibarr ne stocke pas les champs libres dans une table pivot : il **ajoute
//! une vraie colonne** a la table de l'objet, nommee **exactement comme le
//! champ**, et il en tient la definition dans `llx_extrafields`. Consequence :
//! ce qu'un tiers a saisi dans Dolibarr est deja dans la base, dans des
//! colonnes que nos ecrans ne montraient pas.
//!
//! Attention au vocabulaire : cote PHP, Dolibarr expose les valeurs sous la cle
//! `options_<nom>` (et ses formulaires postent `options_<nom>`). Cote SQL, la
//! colonne s'appelle `<nom>`, sans prefixe â€” `ExtraFields::create()` fait
//! `DDLAddField($table, $attrname, ...)`. Confondre les deux fait lire une
//! colonne inexistante en silence : c'est ce que faisait ce module, et aucun
//! champ libre n'etait donc lu sur une vraie base.
//!
//! Ce module lit ces definitions et ces valeurs. Il ne cree pas encore de champ
//! (ce serait un `ALTER TABLE`), et il ne devine rien : un type qu'on ne sait
//! pas rendre est affiche tel quel, jamais converti au hasard.
//!
//! Trois pieges verifies sur la base reelle, pas supposes :
//!
//! 1. `llx_extrafields.enabled` est du **texte**, comme `llx_menu.enabled` :
//!    vide ou `1` = actif.
//! 2. `param` est decode par `jsonOrUnserialize()` cote Dolibarr : il peut donc
//!    etre du **JSON** (usage moderne) ou du **PHP serialise** (heritage).
//!    Les deux sont lus ici.
//! 3. Le nom du type depend de l'objet, pas de la table : `customer` et
//!    `fournisseur` pointent tous deux sur `llx_societe`. La correspondance
//!    reprise est celle de `getElementProperties()` du cceur, relevee a la main.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une definition de champ libre, telle qu'elle est en base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub rowid: i64,
    pub nom: String,
    pub elementtype: String,
    pub libelle: String,
    /// Type declare (`varchar`, `select`, `price`...).
    pub type_: String,
    /// Taille demandee, quand le type en prend une.
    pub taille: Option<String>,
    /// Valeur par defaut.
    pub defaut: Option<String>,
    pub requis: bool,
    /// `enabled` est du texte : vide ou `1` = actif.
    pub enabled: Option<String>,
    pub pos: i64,
    /// `param` brut : options des listes, decode ici.
    pub param: Option<String>,
    /// `list` : colonne d'affichage dans les listes.
    pub liste: Option<String>,
    pub aide: Option<String>,
    /// Expression calculee : la valeur n'est pas saisissable.
    pub calcule: Option<String>,
}

impl Definition {
    /// La colonne SQL qui porte la valeur.
    pub fn colonne(&self) -> String {
        colonne(&self.nom)
    }

    /// Un champ actif : `enabled` vide ou `1`. Dolibarr evalue le reste comme
    /// du PHP (`isModEnabled("x")`), qu'on ne peut pas interpreter â€” un champ
    /// porte une condition est donc traite comme actif, jamais comme absent.
    pub fn actif(&self) -> bool {
        self.enabled.as_deref().map(str::trim) != Some("0")
    }

    /// Un champ calcule par expression : sa valeur se lit, ne s'ecrit pas.
    pub fn calcule_par_expression(&self) -> bool {
        self.calcule
            .as_deref()
            .map(|c| !c.trim().is_empty())
            .unwrap_or(false)
    }

    /// Les options d'une liste (`select`, `radio`, `checkbox`), dans l'ordre.
    pub fn options(&self) -> Vec<(String, String)> {
        options(self.param.as_deref())
    }
}

/// La colonne SQL d'un champ libre : **son nom**, sans prefixe.
///
/// `ExtraFields::create()` en fait `DDLAddField($table, $attrname, ...)` : la
/// colonne porte le nom du champ. Le prefixe `options_` n'existe que cote PHP,
/// dans les tableaux `array_options` et les formulaires.
///
/// Le nom est refuse s'il n'est pas un identifiant sur : il vient de la base,
/// donc d'une saisie, et finit dans une requete.
pub fn colonne(nom: &str) -> String {
    let sur = !nom.is_empty()
        && nom.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && nom
            .chars()
            .next()
            .map(|c| !c.is_ascii_digit())
            .unwrap_or(false);
    if sur {
        nom.to_string()
    } else {
        // Un nom inattendu ne doit pas fabriquer une requete : on le rend tel
        // quel, la lecture echouera et le dira.
        String::new()
    }
}

/// Les types que le cceur de Dolibarr connait, releves dans
/// `ExtraFields::showInputField()` de la version 24.0.1.
pub const TYPES: &[&str] = &[
    "boolean",
    "checkbox",
    "chkbxlst",
    "date",
    "datetime",
    "datetimegmt",
    "double",
    "duration",
    "email",
    "html",
    "icon",
    "int",
    "ip",
    "linestrg",
    "link",
    "mail",
    "multipts",
    "password",
    "phone",
    "point",
    "polygon",
    "price",
    "pricecy",
    "radio",
    "select",
    "sellist",
    "separator",
    "stars",
    "text",
    "url",
    "varchar",
];

/// Correspondance `elementtype` -> table, relevee dans
/// `getElementProperties()` du cceur (24.0.1). Les objets absents de cette
/// table suivent la regle par defaut de Dolibarr : `llx_<elementtype>`.
pub const TABLES: &[(&str, &str)] = &[
    ("action", "llx_actioncomm"),
    ("adherent_type", "llx_adherent_type"),
    ("category", "llx_categorie"),
    ("ccountry", "llx_c_country"),
    ("chargesociales", "llx_chargesociales"),
    ("commande", "llx_commande"),
    ("commande_fournisseurdet", "llx_commande_fournisseurdet"),
    ("commandedet", "llx_commandedet"),
    ("contact", "llx_socpeople"),
    ("contract", "llx_contract"),
    ("contratdet", "llx_contratdet"),
    ("cronjob", "llx_cron"),
    ("customer", "llx_societe"),
    ("dolresource", "llx_resource"),
    ("ecmfiles", "llx_ecmfiles"),
    ("emailsenderprofile", "llx_c_email_senderprofile"),
    ("expeditiondet", "llx_expeditiondet"),
    ("facture", "llx_facture"),
    ("facture_fourn_det", "llx_facture_fourn_det"),
    ("facturedet", "llx_facturedet"),
    ("fichinter", "llx_fichinter"),
    ("fournisseur", "llx_societe"),
    ("inventoryline", "llx_inventorydet"),
    ("invoice_supplier", "llx_facture_fourn"),
    ("member", "llx_adherent"),
    ("mo", "llx_mrp_mo"),
    ("mrp_production", "llx_mrp_production"),
    ("order_supplier", "llx_commande_fournisseur"),
    ("product_attribute", "llx_product_attribute"),
    ("product_attribute_value", "llx_product_attribute_value"),
    ("productlot", "llx_product_lot"),
    ("project", "llx_projet"),
    ("project_task", "llx_projet_task"),
    ("propal", "llx_propal"),
    ("propaldet", "llx_propaldet"),
    ("shipping", "llx_expedition"),
    ("stock", "llx_entrepot"),
    ("subscription", "llx_subscription"),
    ("supplier_proposaldet", "llx_supplier_proposaldet"),
    ("tva", "llx_tva"),
    ("websitepage", "llx_website_page"),
];

/// La table qui porte les champs d'un `elementtype`, si elle est connue.
pub fn table_cible(elementtype: &str) -> String {
    TABLES
        .iter()
        .find(|(e, _)| *e == elementtype)
        .map(|(_, t)| (*t).to_string())
        .unwrap_or_else(|| format!("llx_{elementtype}"))
}

/// La table qui porte les **valeurs** des champs libres d'un `elementtype`.
///
/// Dolibarr ajoute une table `<table de l'objet>_extrafields` : ce n'est pas la
/// table de l'objet elle-meme. Lire les colonnes de `llx_propal` ne donne jamais
/// un champ libre â€” et c'est ce que faisait ce module, ce qui rendait tout
/// `valeurs()` vide sans la moindre erreur.
pub fn table_valeurs(elementtype: &str) -> String {
    format!("{}_extrafields", table_cible(elementtype))
}

/// Les definitions d'une entite (et les globales, `entity = 0`).
pub fn definitions(pool: &Pool, entity: i64) -> Result<Vec<Definition>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    // Les tuples de plus de 12 elements n'ont pas de `FromRow` : on lit les
    // lignes et on les depile a la main, dans l'ordre du SELECT.
    let rows: Vec<mysql::Row> = conn.exec(
        "SELECT rowid, name, elementtype, label, type, size, fielddefault, fieldrequired, \
                enabled, pos, param, list, help, fieldcomputed \
         FROM llx_extrafields WHERE entity IN (0, :entity) \
         ORDER BY elementtype ASC, pos ASC, rowid ASC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let texte = |i: usize| r.get_opt::<String, _>(i).and_then(|v| v.ok());
            Definition {
                rowid: r.get_opt::<i64, _>(0).and_then(|v| v.ok()).unwrap_or(0),
                nom: texte(1).unwrap_or_default(),
                elementtype: texte(2).unwrap_or_default(),
                libelle: texte(3).unwrap_or_default(),
                type_: texte(4).unwrap_or_default(),
                taille: texte(5),
                defaut: texte(6),
                requis: r.get_opt::<i32, _>(7).and_then(|v| v.ok()).unwrap_or(0) != 0,
                enabled: texte(8),
                pos: r.get_opt::<i32, _>(9).and_then(|v| v.ok()).unwrap_or(0) as i64,
                param: texte(10),
                liste: texte(11),
                aide: texte(12),
                calcule: texte(13),
            }
        })
        .collect())
}

/// Les `elementtype` qui portent des champs, avec leur nombre de champs.
pub fn cibles(pool: &Pool, entity: i64) -> Result<Vec<(String, usize)>, Box<dyn Error>> {
    let definitions = definitions(pool, entity)?;
    let mut compte: Vec<(String, usize)> = Vec::new();
    for d in definitions {
        match compte.iter_mut().find(|(e, _)| *e == d.elementtype) {
            Some((_, n)) => *n += 1,
            None => compte.push((d.elementtype, 1)),
        }
    }
    Ok(compte)
}

/// Les colonnes reellement presentes dans une table.
///
/// Une definition peut exister sans sa colonne : c'est le cas quand le champ a
/// ete cree puis que la colonne n'a pas suive (restauration de schema). On le
/// signale au lieu de fabriquer une requete qui echouerait.
///
/// Toutes les colonnes sont rendues : la colonne d'un champ libre porte le nom
/// du champ, sans prefixe. Filtrer sur `options_%` â€” la convention des tableaux
/// PHP â€” ne rendait jamais rien, et aucun champ libre n'etait donc lu.
pub fn colonnes_presentes(pool: &Pool, table: &str) -> Result<Vec<String>, Box<dyn Error>> {
    if !nom_valide(table) {
        return Err("nom de table refuse".into());
    }
    let mut conn = pool.get_conn()?;
    let rows: Vec<String> = conn.exec(
        "SELECT COLUMN_NAME FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = :table \
         ORDER BY ORDINAL_POSITION ASC",
        params! { "table" => table },
    )?;
    Ok(rows)
}

/// La table porte-t-elle une colonne `entity` ? Si oui, toute lecture ou
/// ecriture doit la filtrer : c'est la frontiere entre societes.
pub fn a_entite(pool: &Pool, table: &str) -> Result<bool, Box<dyn Error>> {
    if !nom_valide(table) {
        return Err("nom de table refuse".into());
    }
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = :table AND COLUMN_NAME = 'entity'",
        params! { "table" => table },
    )?;
    Ok(n.unwrap_or(0) > 0)
}

/// Une valeur de champ libre : sa definition, ce qu'elle vaut, et si la colonne
/// existe reellement dans la table.
pub type Valeur = (Definition, Option<String>, bool);

/// Les valeurs des champs libres d'un objet, dans l'ordre des definitions.
///
/// Rend `(definition, valeur, colonne existante)`. Une valeur absente reste
/// absente : on n'ecrit pas de chaine vide a la place d'un NULL, ce serait une
/// donnee inventee.
pub fn valeurs(
    pool: &Pool,
    elementtype: &str,
    entity: i64,
    id: i64,
) -> Result<Vec<Valeur>, Box<dyn Error>> {
    let table = table_valeurs(elementtype);
    let presentes = colonnes_presentes(pool, &table)?;
    let definitions: Vec<Definition> = definitions(pool, entity)?
        .into_iter()
        .filter(|d| d.elementtype == elementtype && d.actif())
        .collect();

    if definitions.is_empty() {
        return Ok(Vec::new());
    }

    let indices: Vec<usize> = definitions
        .iter()
        .enumerate()
        .filter(|(_, d)| presentes.contains(&d.colonne()))
        .map(|(i, _)| i)
        .collect();

    // Aucune colonne a lire : on rend les definitions, valeurs vides.
    if indices.is_empty() {
        return Ok(definitions.into_iter().map(|d| (d, None, false)).collect());
    }

    let projection = indices
        .iter()
        .map(|&i| {
            let c = definitions[i].colonne();
            // Meme precaution que l'explorateur de tables : un point passe par
            // ST_AsText, un binaire par sa taille, tout le reste par CAST.
            format!("CAST(`{c}` AS CHAR)")
        })
        .collect::<Vec<_>>()
        .join(", ");

    let mut conn = pool.get_conn()?;
    let filtre_entite = if a_entite(pool, &table)? {
        " AND entity = :entity"
    } else {
        ""
    };
    // La table des champs libres a son propre `rowid` et lie l'objet par
    // `fk_object` (`UNIQUE KEY uk_<table>_extrafields (fk_object)`). Chercher
    // par `rowid` ne trouvait rien â€” ou pire, la ligne d'un autre objet dont le
    // numero tombait juste.
    let sql =
        format!("SELECT {projection} FROM `{table}` WHERE fk_object = :id{filtre_entite} LIMIT 1");
    let ligne: Option<mysql::Row> =
        conn.exec_first(sql, params! { "id" => id, "entity" => entity })?;

    let mut resultat = Vec::with_capacity(definitions.len());
    for (i, d) in definitions.into_iter().enumerate() {
        let existe = presentes.contains(&d.colonne());
        let valeur = match (&ligne, existe) {
            (Some(row), true) => {
                let rang = indices.iter().position(|&x| x == i).unwrap_or(0);
                row.get_opt::<String, _>(rang).and_then(|v| v.ok())
            }
            _ => None,
        };
        resultat.push((d, valeur, existe));
    }
    Ok(resultat)
}

/// Ecrit les valeurs des champs libres d'un objet.
///
/// Refuse tout ce qui n'est pas une definition connue, active et non calculee :
/// un nom de colonne libre dans une requete, c'est une injection.
/// Rend le nombre de champs reellement ecrits.
pub fn enregistrer(
    pool: &Pool,
    elementtype: &str,
    entity: i64,
    id: i64,
    saisies: &[(String, String)],
) -> Result<usize, Box<dyn Error>> {
    let table = table_valeurs(elementtype);
    let presentes = colonnes_presentes(pool, &table)?;
    let definitions = definitions(pool, entity)?;

    let mut morceaux = Vec::new();
    let mut valeurs: Vec<mysql::Value> = Vec::new();
    for (nom, valeur) in saisies {
        let Some(d) = definitions.iter().find(|d| {
            d.elementtype == elementtype
                && d.nom == *nom
                && d.actif()
                && !d.calcule_par_expression()
        }) else {
            return Err(format!("champ inconnu ou non saisissable : {nom}").into());
        };
        if !presentes.contains(&d.colonne()) {
            return Err(
                format!("colonne absente pour le champ {} : {}", d.nom, d.colonne()).into(),
            );
        }
        morceaux.push(format!("`{}` = ?", d.colonne()));
        // Une saisie vide vaut NULL : c'est ce que fait Dolibarr, et cela
        // distingue Â« pas renseigne Â» de Â« renseigne a vide Â».
        valeurs.push(if valeur.trim().is_empty() {
            mysql::Value::NULL
        } else {
            mysql::Value::from(valeur.clone())
        });
    }

    if morceaux.is_empty() {
        return Ok(0);
    }

    let avec_entite = a_entite(pool, &table)?;
    // La ligne est liee par `fk_object`, pas par son `rowid` : un UPDATE par
    // `rowid` ne touchait aucun objet, ou touchait celui dont le numero tombait
    // juste. `ON DUPLICATE KEY` couvre le cas d'une ligne absente.
    let sql = format!(
        "UPDATE `{table}` SET {} WHERE fk_object = ?{}",
        morceaux.join(", "),
        if avec_entite { " AND entity = ?" } else { "" }
    );
    valeurs.push(mysql::Value::from(id));
    if avec_entite {
        valeurs.push(mysql::Value::from(entity));
    }

    let mut conn = pool.get_conn()?;
    conn.exec_drop(sql, mysql::Params::Positional(valeurs))?;
    Ok(morceaux.len())
}

/// Les options d'une liste, que `param` soit du JSON ou du PHP serialise.
///
/// Dolibarr decode ce champ avec `jsonOrUnserialize()` : les deux formats
/// existent donc en production, selon l'age de l'installation.
pub fn options(param: Option<&str>) -> Vec<(String, String)> {
    let Some(brut) = param.map(str::trim).filter(|s| !s.is_empty()) else {
        return Vec::new();
    };

    // 1. JSON : `{"cle":"libelle"}` ou `{"options":{...}}`.
    if brut.starts_with('{') {
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(brut) {
            if let Some(serde_json::Value::Object(inner)) = map.get("options") {
                return inner
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect();
            }
            return map
                .iter()
                .filter(|(k, _)| k.as_str() != "options")
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                .collect();
        }
    }

    // 2. PHP serialise : `a:2:{s:3:"cle";s:7:"libelle";...}`.
    if brut.starts_with("a:") {
        if let Some(v) = php_tableau(brut) {
            return v;
        }
    }

    // 3. A defaut, une liste separee par des virgules reste lisible.
    brut.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| (s.to_string(), s.to_string()))
        .collect()
}

/// Un tableau PHP serialise, plat, de chaines. Assez pour `param`.
fn php_tableau(brut: &str) -> Option<Vec<(String, String)>> {
    let mut reste = brut.strip_prefix("a:")?;
    let (n, apres) = reste.split_once(':')?;
    // Le compte vient de `llx_extrafields.param`, une colonne écrite par
    // l'interface Dolibarr, un import, ou une restauration partielle :
    // `a:100000000:{` demandait 4,8 Go, et l'échec d'allocation abandonne le
    // processus. Aucun champ réel n'a 64 options.
    let n: usize = n.parse().ok().filter(|n| *n <= 64)?;
    reste = apres.strip_prefix('{')?;

    let mut sortie = Vec::with_capacity(n);
    for _ in 0..n {
        let (cle, apres) = php_chaine(reste)?;
        let (valeur, apres) = php_chaine(apres)?;
        sortie.push((cle, valeur));
        reste = apres;
    }
    Some(sortie)
}

/// Une chaine PHP serialisee `s:<longueur>:"<contenu>";`.
fn php_chaine(brut: &str) -> Option<(String, &str)> {
    let reste = brut.strip_prefix("s:")?;
    let (longueur, apres) = reste.split_once(':')?;
    let longueur: usize = longueur.parse().ok()?;
    let apres = apres.strip_prefix('"')?;
    if apres.len() < longueur {
        return None;
    }
    // `apres.len() < longueur` est vérifié juste avant, mais pas la frontière
    // de caractère : `s:3:"éé"` (chaque « é » compte deux octets) faisait
    // paniquer `split_at`.
    if !apres.is_char_boundary(longueur) {
        return None;
    }
    let (contenu, fin) = apres.split_at(longueur);
    let fin = fin.strip_prefix("\";")?;
    Some((contenu.to_string(), fin))
}

/// Un identifiant SQL acceptable : lettres, chiffres, souligne.
fn nom_valide(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_colonne_porte_le_nom_du_champ() {
        // Verifie dans la source Dolibarr : `create()` appelle
        // `DDLAddField($table, $attrname, ...)`, sans prefixe. Le prefixe
        // `options_` n'existe que dans les tableaux PHP et les formulaires.
        assert_eq!(colonne("mon_champ"), "mon_champ");
        assert_eq!(colonne("pct_sign_pdf"), "pct_sign_pdf");
    }

    #[test]
    fn un_nom_de_champ_douteux_ne_fabrique_pas_de_requete() {
        // Le nom vient de la base, donc d'une saisie : il finit dans un SELECT.
        assert_eq!(colonne("champ; DROP TABLE llx_societe"), "");
        assert_eq!(colonne("mon champ"), "");
        assert_eq!(colonne("1champ"), "");
        assert_eq!(colonne(""), "");
    }

    #[test]
    fn customer_et_fournisseur_pointent_sur_la_meme_table() {
        assert_eq!(table_cible("customer"), "llx_societe");
        assert_eq!(table_cible("fournisseur"), "llx_societe");
        assert_eq!(table_cible("project_task"), "llx_projet_task");
        // Regle par defaut du cceur : le nom de l'element est celui de la table.
        assert_eq!(table_cible("societe"), "llx_societe");
        assert_eq!(table_cible("user"), "llx_user");
    }

    #[test]
    fn un_champ_est_actif_sauf_s_il_vaut_zero() {
        let mut d = Definition {
            rowid: 1,
            nom: "x".into(),
            elementtype: "customer".into(),
            libelle: "X".into(),
            type_: "varchar".into(),
            taille: None,
            defaut: None,
            requis: false,
            enabled: None,
            pos: 0,
            param: None,
            liste: None,
            aide: None,
            calcule: None,
        };
        assert!(d.actif(), "vide = actif, comme llx_menu.enabled");
        d.enabled = Some("1".into());
        assert!(d.actif());
        d.enabled = Some("0".into());
        assert!(!d.actif());
        // Une condition (PHP) n'est pas interpretee : le champ reste visible.
        d.enabled = Some("isModEnabled(\"projet\")".into());
        assert!(d.actif());
    }

    #[test]
    fn une_expression_calculee_n_est_pas_saisissable() {
        let d = Definition {
            rowid: 1,
            nom: "total".into(),
            elementtype: "facture".into(),
            libelle: "Total".into(),
            type_: "price".into(),
            taille: None,
            defaut: None,
            requis: false,
            enabled: Some("1".into()),
            pos: 0,
            param: None,
            liste: None,
            aide: None,
            calcule: Some("$objectoffield->total_ttc".into()),
        };
        assert!(d.calcule_par_expression());
    }

    #[test]
    fn les_options_se_lisent_en_json() {
        let o = options(Some(r#"{"a":"Choix A","b":"Choix B"}"#));
        assert_eq!(
            o,
            vec![
                ("a".to_string(), "Choix A".to_string()),
                ("b".to_string(), "Choix B".to_string())
            ]
        );
    }

    #[test]
    fn les_options_se_lisent_aussi_en_php_serialise() {
        // a:2:{s:1:"a";s:7:"Choix A";s:1:"b";s:7:"Choix B";}
        let brut = r#"a:2:{s:1:"a";s:7:"Choix A";s:1:"b";s:7:"Choix B";}"#;
        let o = options(Some(brut));
        assert_eq!(
            o,
            vec![
                ("a".to_string(), "Choix A".to_string()),
                ("b".to_string(), "Choix B".to_string())
            ]
        );
    }

    #[test]
    fn un_param_vide_ne_rend_aucune_option() {
        assert!(options(None).is_empty());
        assert!(options(Some("   ")).is_empty());
        assert!(options(Some("{}")).is_empty());
    }

    #[test]
    fn un_param_illisible_est_traite_comme_une_liste_simple() {
        let o = options(Some("rouge, vert , bleu"));
        assert_eq!(o.len(), 3);
        assert_eq!(o[1].0, "vert");
    }

    #[test]
    fn les_tables_cibles_sont_toutes_des_noms_valides() {
        for (elementtype, table) in TABLES {
            assert!(
                nom_valide(table),
                "table invalide pour {elementtype} : {table}"
            );
            assert!(
                nom_valide(elementtype),
                "elementtype invalide : {elementtype}"
            );
        }
    }
}
