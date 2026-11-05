//! Utilisateur de la requête courante.
//!
//! `entite::courante()` porte la société ; ce module porte la **personne**.
//! Sans lui, un écran qui doit écrire « qui a créé cette pièce » n'a que trois
//! issues : inventer un identifiant, en choisir un au hasard, ou laisser la
//! colonne vide. Les trois sont fausses, et la troisième est la plus
//! dangereuse : une pièce comptable sans auteur ne s'explique plus.
//!
//! Le contexte est posé par le garde-fou de `main.rs`, exactement comme
//! l'entité : une variable de tâche, donc deux requêtes concurrentes ne peuvent
//! pas se mélanger.

tokio::task_local! {
    static LOGIN: String;
}

/// Le login de la requête en cours, `None` hors requête HTTP (CLI, tests).
///
/// Rend `None` plutôt qu'une chaîne vide : « personne » et « un utilisateur
/// sans nom » ne sont pas la même chose, et un écran doit pouvoir refuser
/// d'écrire une pièce quand il ne sait pas qui agit.
pub fn courant() -> Option<String> {
    LOGIN.try_with(|l| l.clone()).ok()
}

/// Exécute un futur dans le contexte d'un utilisateur.
pub async fn avec<F: std::future::Future>(login: String, futur: F) -> F::Output {
    LOGIN.scope(login, futur).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn le_contexte_porte_le_login() {
        let vu = avec("maxime".to_string(), async { courant() }).await;
        assert_eq!(vu.as_deref(), Some("maxime"));
    }

    #[tokio::test]
    async fn hors_requete_il_n_y_a_personne() {
        assert_eq!(courant(), None);
    }

    #[tokio::test]
    async fn deux_contextes_ne_se_melangent_pas() {
        let a = avec("a".to_string(), async {
            tokio::task::yield_now().await;
            courant()
        });
        let b = avec("b".to_string(), async {
            tokio::task::yield_now().await;
            courant()
        });
        let (a, b) = tokio::join!(a, b);
        assert_eq!(a.as_deref(), Some("a"));
        assert_eq!(b.as_deref(), Some("b"));
    }
}
