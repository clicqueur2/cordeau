//! Modules declaratifs : un fichier JSON suffit, sans recompiler le noyau.
//!
//! Dolibarr charge ses modules en executant du PHP. Rust ne peut pas faire cela
//! sans ABI stable, et Rust n'en a pas. Ce noyau prend donc l'autre chemin,
//! celui que suivent la plupart des modules du Dolistore : **decrire** ce que le
//! module fait (menus, listes, formulaires, droits) et laisser le noyau
//! l'executer. Un module declaratif ne peut pas faire plus que ce que le noyau
//! sait deja faire — mais il le fait sans toucher au binaire.
//!
//! # Ce que le chargeur accepte
//!
//! Un dossier (`$DOLIBARR_MODULES`, sinon `./modules`) de fichiers `.json` :
//!
//! ```json
//! {
//!   "code": "Entretien",
//!   "libelle": "Contrats d'entretien",
//!   "menu": { "cle": "entretien",
//!             "entrees": [ { "cle": "liste", "libelle": "Contrats",
//!                            "lien": "/liste/Entretien" } ] },
//!   "droit": { "id": 9501, "libelle": "Contrats d'entretien" },
//!   "listes": [ { "code": "Entretien", "titre": "Contrats",
//!                 "intro": "…", "sql": "SELECT … WHERE entity = :entity",
//!                 "colonnes": ["N°", "Réf.", "Client"] } ],
//!   "saisies": [ { "code": "Entretien", "table": "llx_entretien",
//!                  "champs": [ { "nom": "ref", "libelle": "Référence",
//!                                "sorte": "texte", "obligatoire": true } ],
//!                  "fixes": [ ["datec", "NOW()"] ],
//!                  "reference": ["ref", "CTR"] } ]
//! }
//! ```
//!
//! # Ce que le chargeur refuse
//!
//! Le SQL vient d'un fichier ecrit par l'administrateur du serveur, pas d'une
//! requete HTTP. Malgre cela il est verifie : un seul `SELECT`, sans
//! commentaire ni point-virgule, avec `:entity` obligatoire pour qu'une liste
//! ne puisse pas deborder d'entite. Les noms de table et de colonne suivent la
//! meme regle que partout dans le noyau.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use db::Pool;
use serde::Deserialize;

use crate::menu::{Entree, Module};
use crate::pages::{ChampSaisie, ListeSimple, Saisie, Sorte};

#[derive(Deserialize)]
struct Fichier {
    code: String,
    libelle: String,
    /// Code du module Dolibarr qui commande l'affichage du menu.
    #[serde(default)]
    dolibarr: Option<String>,
    #[serde(default)]
    menu: Option<Menu>,
    #[serde(default)]
    droit: Option<Droit>,
    #[serde(default)]
    listes: Vec<DescriptionListe>,
    #[serde(default)]
    saisies: Vec<DescriptionSaisie>,
}

#[derive(Deserialize)]
struct Menu {
    cle: String,
    #[serde(default)]
    entrees: Vec<DescriptionEntree>,
}

#[derive(Deserialize)]
struct DescriptionEntree {
    cle: String,
    libelle: String,
    lien: String,
}

#[derive(Deserialize)]
struct Droit {
    id: i64,
    libelle: String,
}

#[derive(Deserialize)]
struct DescriptionListe {
    code: String,
    titre: String,
    #[serde(default)]
    intro: String,
    sql: String,
    colonnes: Vec<String>,
}

#[derive(Deserialize)]
struct DescriptionSaisie {
    code: String,
    table: String,
    #[serde(default = "pk_defaut")]
    pk: String,
    #[serde(default)]
    champs: Vec<DescriptionChamp>,
    #[serde(default)]
    fixes: Vec<(String, String)>,
    #[serde(default)]
    reference: Option<(String, String)>,
}

#[derive(Deserialize)]
struct DescriptionChamp {
    nom: String,
    libelle: String,
    #[serde(default = "sorte_texte")]
    sorte: String,
    #[serde(default)]
    obligatoire: bool,
    #[serde(default)]
    choix: Option<String>,
    #[serde(default)]
    defaut: String,
}

fn sorte_texte() -> String {
    "texte".to_string()
}

fn pk_defaut() -> String {
    "rowid".to_string()
}

struct Charge {
    modules: Vec<&'static Module>,
    /// (prefixe de chemin, droit exige) des listes declaratives.
    droits: Vec<(String, i64)>,
}

/// Ce qu'un fichier rend : son module et, s'il en declare un, son droit avec
/// les prefixes de chemin qu'il ouvre.
type Resultat = (&'static Module, Option<(i64, Vec<String>)>);

static CHARGE: OnceLock<Charge> = OnceLock::new();

/// Charge les modules declaratifs d'un dossier.
///
/// Un fichier malforme est signale et ignore : un module casse ne doit pas
/// empecher le serveur de demarrer.
pub fn charger(pool: &Pool, entity: i64, dossier: &Path) -> Result<usize, String> {
    if CHARGE.get().is_some() {
        return Ok(modules().len());
    }

    let mut modules: Vec<&'static Module> = Vec::new();
    let mut droits: Vec<(String, i64)> = Vec::new();
    let mut vus: BTreeSet<String> = BTreeSet::new();

    if dossier.is_dir() {
        let mut fichiers: Vec<_> = fs::read_dir(dossier)
            .map_err(|e| format!("lecture de {} impossible : {e}", dossier.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
            .collect();
        fichiers.sort();

        for chemin in fichiers {
            match charger_fichier(pool, entity, &chemin, &mut vus) {
                Ok((module, droit)) => {
                    modules.push(module);
                    if let Some((id, prefixes)) = droit {
                        for p in prefixes {
                            droits.push((p, id));
                        }
                    }
                }
                Err(e) => eprintln!("module ignore ({}): {e}", chemin.display()),
            }
        }
    }

    let n = modules.len();
    let _ = CHARGE.set(Charge { modules, droits });
    Ok(n)
}

/// Les modules declaratifs charges, dans l'ordre des fichiers.
pub fn modules() -> &'static [&'static Module] {
    CHARGE.get().map(|c| c.modules.as_slice()).unwrap_or(&[])
}

/// Ce module vient-il d'un fichier ? Il est actif des qu'il est charge : pas de
/// `MAIN_MODULE_*` a semer pour lui.
pub fn est_declare(code: &str) -> bool {
    modules().iter().any(|m| m.code.eq_ignore_ascii_case(code))
}

/// Le droit exige par un chemin declaratif, s'il y en a un.
pub fn droit_du_chemin(chemin: &str) -> Option<i64> {
    CHARGE
        .get()?
        .droits
        .iter()
        .filter(|(prefixe, _)| chemin.starts_with(prefixe.as_str()))
        .max_by_key(|(prefixe, _)| prefixe.len())
        .map(|(_, id)| *id)
}

fn charger_fichier(
    pool: &Pool,
    entity: i64,
    chemin: &Path,
    vus: &mut BTreeSet<String>,
) -> Result<Resultat, String> {
    let texte = fs::read_to_string(chemin).map_err(|e| e.to_string())?;
    let f: Fichier = serde_json::from_str(&texte).map_err(|e| format!("JSON invalide : {e}"))?;

    if !db::divers::identifiant_autorise(&f.code) {
        return Err(format!("code de module invalide : {}", f.code));
    }
    if !vus.insert(f.code.to_uppercase()) {
        return Err(format!("module en double : {}", f.code));
    }

    // --- listes ---
    let mut listes: Vec<ListeSimple> = Vec::new();
    for l in &f.listes {
        if !db::divers::identifiant_autorise(&l.code) {
            return Err(format!("code de liste invalide : {}", l.code));
        }
        sql_accepte(&l.sql)?;
        if l.colonnes.is_empty() {
            return Err(format!("liste {} sans colonne", l.code));
        }
        let colonnes: Vec<&'static str> = l.colonnes.iter().map(|c| fuite(c)).collect();
        listes.push(ListeSimple {
            code: fuite(&l.code),
            titre: fuite(&l.titre),
            intro: fuite(&l.intro),
            sql: fuite(&l.sql),
            colonnes: Box::leak(colonnes.into_boxed_slice()),
        });
    }

    // --- formulaires ---
    let mut saisies: Vec<Saisie> = Vec::new();
    for s in &f.saisies {
        if !db::divers::identifiant_autorise(&s.table) {
            return Err(format!("table invalide : {}", s.table));
        }
        let mut champs: Vec<ChampSaisie> = Vec::new();
        for c in &s.champs {
            if !db::divers::identifiant_autorise(&c.nom) {
                return Err(format!("colonne invalide : {}", c.nom));
            }
            champs.push(ChampSaisie {
                nom: fuite(&c.nom),
                libelle: fuite(&c.libelle),
                sorte: sorte(&c.sorte)?,
                obligatoire: c.obligatoire,
                choix: c.choix.as_deref().map(fuite),
                defaut: fuite(&c.defaut),
            });
        }
        let mut fixes: Vec<(&'static str, &'static str)> = Vec::new();
        for (colonne, expr) in &s.fixes {
            if !db::divers::identifiant_autorise(colonne) {
                return Err(format!("colonne invalide dans fixes : {colonne}"));
            }
            if !db::divers::expression_autorisee(expr) {
                return Err(format!(
                    "expression refusee : {expr} (acceptees : {:?})",
                    db::divers::expressions_autorisees()
                ));
            }
            fixes.push((fuite(colonne), fuite(expr)));
        }
        let reference = match &s.reference {
            Some((colonne, prefixe)) => {
                if !db::divers::identifiant_autorise(colonne) {
                    return Err(format!("colonne de reference invalide : {colonne}"));
                }
                Some((fuite(colonne), fuite(prefixe)))
            }
            None => None,
        };
        if !db::divers::identifiant_autorise(&s.pk) {
            return Err(format!("cle primaire invalide : {}", s.pk));
        }
        saisies.push(Saisie {
            code: fuite(&s.code),
            table: fuite(&s.table),
            champs: Box::leak(champs.into_boxed_slice()),
            fixes: Box::leak(fixes.into_boxed_slice()),
            reference,
            pk: fuite(&s.pk),
        });
    }

    // On enregistre les ecrans avant le menu : une entree de menu qui pointe
    // vers une liste inexistante est une page morte.
    crate::pages::enregistrer_listes(listes);
    crate::pages::enregistrer_saisies(saisies);

    // --- menu ---
    let mut prefixes: Vec<String> = vec![format!("/liste/{}", f.code)];
    let module = match &f.menu {
        Some(m) => {
            let mut entrees: Vec<Entree> = Vec::new();
            for e in &m.entrees {
                if !e.lien.starts_with('/') {
                    return Err(format!("lien de menu invalide : {}", e.lien));
                }
                // Le droit couvre l'ecran de tete de l'entree.
                let tete = e
                    .lien
                    .split(['?', '#'])
                    .next()
                    .unwrap_or(&e.lien)
                    .split('/')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join("/");
                if tete.len() > 1 {
                    prefixes.push(tete);
                }
                entrees.push(Entree {
                    cle: fuite(&e.cle),
                    libelle: fuite(&e.libelle),
                    lien: fuite(&e.lien),
                });
            }
            let module = Module {
                cle: fuite(&m.cle),
                libelle: fuite(&f.libelle),
                code: fuite(f.dolibarr.as_deref().unwrap_or(&f.code)),
                entrees: Box::leak(entrees.into_boxed_slice()),
            };
            crate::menu::enregistrer(module);
            crate::menu::modules()
                .iter()
                .find(|x| x.cle == fuite(&m.cle))
                .copied()
                .ok_or("module non enregistre")?
        }
        None => Box::leak(Box::new(Module {
            cle: fuite(&f.code),
            libelle: fuite(&f.libelle),
            code: fuite(&f.code),
            entrees: &[],
        })),
    };

    // --- droit ---
    let droit = match &f.droit {
        Some(d) => {
            db::droit::assurer_un(pool, entity, d.id, &d.libelle)
                .map_err(|e| format!("droit non ecrit : {e}"))?;
            prefixes.sort();
            prefixes.dedup();
            Some((d.id, prefixes))
        }
        None => None,
    };

    Ok((module, droit))
}

/// Un `SELECT`, une seule instruction, borne a l'entite.
fn sql_accepte(sql: &str) -> Result<(), String> {
    let s = sql.trim();
    let bas = s.to_lowercase();
    if !bas.starts_with("select") {
        return Err("une liste declarative ne peut que lire (SELECT)".to_string());
    }
    if s.contains(';') || s.contains("--") || s.contains("/*") {
        return Err("SQL refuse : une seule instruction, sans commentaire".to_string());
    }
    if !s.contains(":entity") {
        return Err("SQL refuse : :entity obligatoire (cloisonnement des entites)".to_string());
    }
    Ok(())
}

fn sorte(s: &str) -> Result<Sorte, String> {
    match s.to_lowercase().as_str() {
        "texte" => Ok(Sorte::Texte),
        "nombre" => Ok(Sorte::Nombre),
        "date" => Ok(Sorte::Date),
        "entier" => Ok(Sorte::Entier),
        autre => Err(format!("sorte inconnue : {autre}")),
    }
}

/// Une chaine venue du fichier vit aussi longtemps que le serveur : les
/// registres sont en `&'static str` et ne sont charges qu'une fois.
fn fuite(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_liste_ne_peut_que_lire() {
        assert!(sql_accepte("SELECT ref FROM llx_x WHERE entity = :entity").is_ok());
        assert!(sql_accepte("  select ref FROM llx_x WHERE entity = :entity").is_ok());
        assert!(sql_accepte("DELETE FROM llx_x WHERE entity = :entity").is_err());
        assert!(sql_accepte("UPDATE llx_x SET a = 1 WHERE entity = :entity").is_err());
    }

    #[test]
    fn le_sql_ne_fuit_pas_de_l_entite() {
        assert!(sql_accepte("SELECT ref FROM llx_x").is_err());
    }

    #[test]
    fn le_sql_ne_porte_qu_une_instruction() {
        assert!(
            sql_accepte("SELECT 1 FROM llx_x WHERE entity = :entity; DROP TABLE llx_user").is_err()
        );
        assert!(
            sql_accepte("SELECT 1 -- puis n'importe quoi\nFROM llx_x WHERE entity = :entity")
                .is_err()
        );
        assert!(sql_accepte("SELECT /* x */ 1 FROM llx_x WHERE entity = :entity").is_err());
    }

    #[test]
    fn les_sortes_sont_connues() {
        assert!(matches!(sorte("nombre"), Ok(Sorte::Nombre)));
        assert!(matches!(sorte("DATE"), Ok(Sorte::Date)));
        assert!(sorte("binaire").is_err());
    }
}
