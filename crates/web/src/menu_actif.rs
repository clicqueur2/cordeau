//! Modules actifs, lus une fois en base et tenus a jour en memoire.
//!
//! La barre de menus est construite a chaque requete : elle a besoin de savoir
//! quels modules sont actives sans ouvrir une connexion MySQL par page. On
//! charge donc l'ensemble au demarrage, et on le recharge apres chaque
//! activation ou desactivation.

use std::collections::BTreeSet;
use std::sync::RwLock;

use db::Pool;

static ACTIFS: RwLock<Option<BTreeSet<String>>> = RwLock::new(None);

/// Relit les modules actives dans `llx_const`.
pub fn charger(pool: &Pool, entity: i64) -> Result<(), Box<dyn std::error::Error>> {
    let actifs = db::module::actifs(pool, entity)?;
    *ACTIFS.write().unwrap_or_else(|e| e.into_inner()) = Some(actifs);
    Ok(())
}

/// Le module est-il active ? Tant que rien n'est charge, on considere que oui
/// (une barre complete vaut mieux qu'une barre vide si la base est absente).
pub fn est_actif(code: &str) -> bool {
    match ACTIFS.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(actifs) => actifs.contains(&code.to_uppercase()),
        None => true,
    }
}
