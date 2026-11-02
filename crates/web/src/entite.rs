//! Entite courante (multi-societe), portee par la requete.
//!
//! Dolibarr range chaque ligne par `entity` : c'est le seul mecanisme dont on a
//! besoin pour tenir plusieurs societes dans une base. L'entite est choisie a la
//! connexion (elle vient de la session) et stockee dans une variable de tache
//! le temps de la requete : deux requetes concurrentes sur deux societes
//! differentes ne peuvent pas se melanger.

use std::error::Error;
use std::sync::RwLock;

static LISTE: RwLock<Vec<(i64, String)>> = RwLock::new(Vec::new());

tokio::task_local! {
    static ENTITE: i64;
}

/// Entite de la requete en cours. 1 par defaut (hors requete HTTP, CLI, tests).
pub fn courante() -> i64 {
    ENTITE.try_with(|e| *e).unwrap_or(1)
}

/// Execute un futur dans le contexte d'une entite.
pub async fn avec<F: std::future::Future>(entite: i64, futur: F) -> F::Output {
    ENTITE.scope(entite, futur).await
}

/// Recharge la liste des entites (au demarrage et apres une creation).
pub fn charger(pool: &db::Pool) -> Result<(), Box<dyn Error>> {
    let liste = db::company::entites(pool)?;
    *LISTE.write().unwrap_or_else(|e| e.into_inner()) = liste;
    Ok(())
}

/// Entites connues, telles que lues au demarrage.
pub fn liste() -> Vec<(i64, String)> {
    LISTE.read().unwrap_or_else(|e| e.into_inner()).clone()
}
