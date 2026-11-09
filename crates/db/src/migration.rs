//! Planifier une montée de version Dolibarr, sans rien exécuter.
//!
//! Dolibarr migre : ce sont ses 31 scripts (`install/mysql/migration/`) qui font
//! passer une base de la 3.0 à la 24. Cordeau n'a pas à les réécrire — il doit
//! savoir **lesquels** appliquer, dans quel ordre, et s'arrêter net si la chaîne
//! est trouée.
//!
//! Ce module ne touche pas à la base. Il prend la version de départ, la version
//! visée, la liste des fichiers disponibles, et rend le chemin. C'est la partie
//! qu'on peut tester sans serveur — et celle où une erreur d'ordre coûterait une
//! base entière.

use std::error::Error;

use mysql::prelude::*;
use mysql::Pool;

/// Une étape : le fichier à jouer, et les deux versions qu'il relie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Etape {
    pub fichier: String,
    pub de: String,
    pub vers: String,
}

impl Etape {
    /// Le nom `12.0.0-13.0.0.sql` devient `(12.0.0, 13.0.0)`.
    ///
    /// Un nom qui ne suit pas ce format n'est pas une étape de migration :
    /// `repair.sql` en est un exemple, et il n'a pas sa place dans un chemin
    /// ordonné.
    pub fn depuis_nom(fichier: &str) -> Option<Etape> {
        let base = fichier.strip_suffix(".sql")?;
        let (de, vers) = base.split_once('-')?;
        let plausible = |v: &str| {
            !v.is_empty()
                && v.split('.').count() >= 2
                && v.split('.').all(|p| p.parse::<u32>().is_ok())
        };
        if plausible(de) && plausible(vers) {
            Some(Etape {
                fichier: fichier.to_string(),
                de: de.to_string(),
                vers: vers.to_string(),
            })
        } else {
            None
        }
    }
}

/// Le chemin complet entre deux versions, ou l'endroit où il se coupe.
///
/// Rend les étapes **dans l'ordre**, de la version de départ à la version visée.
/// Une version de départ inconnue, une version visée jamais atteinte ou un trou
/// dans la chaîne rendent une erreur explicite : mieux vaut refuser que migrer
/// à moitié.
pub fn chemin(
    depuis: &str,
    vers: &str,
    fichiers: &[String],
) -> Result<Vec<Etape>, Box<dyn std::error::Error>> {
    let mut etapes: Vec<Etape> = fichiers
        .iter()
        .filter_map(|f| Etape::depuis_nom(f))
        .collect();
    etapes.sort_by(|a, b| comparer(&a.de, &b.de));

    if !etapes.iter().any(|e| e.de == depuis) {
        return Err(format!(
            "aucune migration ne part de la version {depuis} : les scripts disponibles commencent à {}",
            etapes.first().map(|e| e.de.as_str()).unwrap_or("(aucun)")
        )
        .into());
    }

    let mut restant = etapes
        .into_iter()
        .filter(|e| comparer(&e.de, depuis) >= std::cmp::Ordering::Equal)
        .peekable();

    let mut sortie = Vec::new();
    let mut courante = depuis.to_string();
    while courante != *vers {
        let Some(e) = restant.next() else {
            return Err(
                format!("la chaîne s'arrête à {courante} : aucun script ne mène à {vers}").into(),
            );
        };
        if e.de != courante {
            return Err(format!(
                "trou dans la chaîne : {courante} n'a pas de script, le suivant part de {}",
                e.de
            )
            .into());
        }
        courante = e.vers.clone();
        sortie.push(e);
    }
    Ok(sortie)
}

/// Compare deux versions point par point, sans passer par des nombres flottants.
fn comparer(a: &str, b: &str) -> std::cmp::Ordering {
    let decouper = |v: &str| -> Vec<u32> { v.split('.').filter_map(|p| p.parse().ok()).collect() };
    let (x, y) = (decouper(a), decouper(b));
    for i in 0..x.len().max(y.len()) {
        let g = x.get(i).copied().unwrap_or(0);
        let d = y.get(i).copied().unwrap_or(0);
        match g.cmp(&d) {
            std::cmp::Ordering::Equal => continue,
            autre => return autre,
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fichiers() -> Vec<String> {
        vec![
            "3.0.0-3.1.0.sql".into(),
            "3.1.0-3.2.0.sql".into(),
            "23.0.0-24.0.0.sql".into(),
            "22.0.0-23.0.0.sql".into(),
            "repair.sql".into(),
        ]
    }

    #[test]
    fn un_nom_de_fichier_donne_une_etape() {
        let e = Etape::depuis_nom("23.0.0-24.0.0.sql").unwrap();
        assert_eq!((e.de.as_str(), e.vers.as_str()), ("23.0.0", "24.0.0"));
        assert!(Etape::depuis_nom("repair.sql").is_none());
    }

    #[test]
    fn le_chemin_est_ordonne_et_s_arrete_a_la_version_visee() {
        let c = chemin("22.0.0", "24.0.0", &fichiers()).unwrap();
        assert_eq!(
            c.iter().map(|e| e.fichier.as_str()).collect::<Vec<_>>(),
            ["22.0.0-23.0.0.sql", "23.0.0-24.0.0.sql"]
        );
    }

    #[test]
    fn une_version_de_depart_inconnue_est_refusee() {
        let err = chemin("19.0.0", "24.0.0", &fichiers())
            .unwrap_err()
            .to_string();
        assert!(err.contains("aucune migration ne part"), "{err}");
    }

    #[test]
    fn un_trou_dans_la_chaine_est_refuse() {
        // On retire l'étape 23 → 24 : la chaîne ne peut pas atteindre 24.
        let mut f = fichiers();
        f.retain(|x| x != "23.0.0-24.0.0.sql");
        let err = chemin("22.0.0", "24.0.0", &f).unwrap_err().to_string();
        assert!(err.contains("la chaîne s'arrête"), "{err}");
    }

    #[test]
    fn les_versions_se_comparent_sans_flottants() {
        assert_eq!(comparer("3.10.0", "3.9.0"), std::cmp::Ordering::Greater);
        assert_eq!(comparer("24.0.0", "24.0.0"), std::cmp::Ordering::Equal);
        assert_eq!(comparer("9.0.0", "10.0.0"), std::cmp::Ordering::Less);
    }
}

/// Joue des instructions SQL dans **une seule transaction**.
///
/// Une erreur annule tout et remonte : jamais de demi-migration, qui laisserait
/// la base ni dans l'ancienne version ni dans la nouvelle.
///
/// Le decoupage n'est pas ici : il appartient a `schema::prepare_file`, deja
/// verifie sur les scripts de Dolibarr 12 (307 tables sur 307). Ce module ne
/// connait que l'execution.
pub fn executer(pool: &Pool, instructions: &[String]) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    conn.query_drop("START TRANSACTION")?;
    for (i, instruction) in instructions.iter().enumerate() {
        if let Err(e) = conn.query_drop(instruction.as_str()) {
            // Dolibarr joue ces scripts avec `$db->query()`, qui **ignore** les
            // erreurs : une base 12.0.5 porte deja des colonnes que le script
            // 12→13 s'apprete a ajouter, parce qu'un correctif de la 12 les a
            // introduites. Arreter la-dessus rendrait toute migration
            // impossible.
            //
            // On tolere donc trois codes, et trois seulement, parce qu'ils
            // disent « c'est deja fait » et non « c'est casse » :
            //   1050 table deja la, 1051 table absente, 1060 colonne en double,
            //   1061 index en double, 1091 cle absente.
            // Tout le reste arrete la migration.
            let deja_fait = match &e {
                mysql::Error::MySqlError(me) => matches!(me.code, 1050 | 1051 | 1060 | 1061 | 1091),
                _ => false,
            };
            if deja_fait {
                continue;
            }
            let _ = conn.query_drop("ROLLBACK");
            return Err(format!("instruction {} refusee : {e}", i + 1).into());
        }
    }
    conn.query_drop("COMMIT")?;
    Ok(())
}
