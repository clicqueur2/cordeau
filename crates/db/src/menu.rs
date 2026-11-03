//! Menus lus depuis la base : la disposition de Dolibarr, pas la notre.
//!
//! Dolibarr ne code pas sa barre dans le PHP : il la lit dans `llx_menu`, table
//! construite par `menuLoad()`. Reproduire « exactement au meme endroit » ne
//! demande donc pas de recopier du code, mais de lire cette table et de rendre
//! l'arbre tel qu'il est.
//!
//! Tant que la table est vide (installation neuve), l'appelant garde ses menus
//! par defaut : on ne casse rien, on se prepare.

use std::error::Error;

use mysql::prelude::*;
use mysql::{params, Pool};

/// Une entree de menu, telle que Dolibarr la stocke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entree {
    pub rowid: i64,
    /// 0 pour un menu de tete.
    pub pere: i64,
    pub position: i64,
    pub titre: String,
    pub url: String,
    /// Code du menu de tete (`mainmenu`), vide pour un menu de tete lui-meme.
    pub mainmenu: String,
    /// 0 = menu de tete, 1 = entree de gauche.
    pub niveau: i32,
    /// `top` ou `left` : a quel endroit Dolibarr l'affiche.
    pub type_menu: String,
    pub actif: bool,
}

/// Une ligne brute de `llx_menu`, avant mise en forme.
type LigneMenu = (
    i64,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<i64>,
);

/// Lit l'arbre des menus d'une entite, dans l'ordre d'affichage.
///
/// Aucune ecriture : cette table appartient a Dolibarr, on ne fait que la lire.
pub fn lire(pool: &Pool, entity: i64) -> Result<Vec<Entree>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<LigneMenu> = conn.exec(
        "SELECT rowid, fk_menu, position, titre, url, mainmenu, niveau, type, enabled \
             FROM llx_menu WHERE entity = :entity \
             ORDER BY fk_menu ASC, position ASC, rowid ASC",
        params! { "entity" => entity },
    )?;

    Ok(rows
        .into_iter()
        .map(
            |(rowid, pere, position, titre, url, mainmenu, niveau, type_menu, enabled)| Entree {
                rowid,
                pere: pere.unwrap_or(0),
                position: position.unwrap_or(0),
                titre: titre.unwrap_or_default(),
                url: url.unwrap_or_default(),
                mainmenu: mainmenu.unwrap_or_default(),
                niveau: niveau.unwrap_or(0) as i32,
                type_menu: type_menu.unwrap_or_default(),
                actif: enabled.map(|e| e > 0).unwrap_or(true),
            },
        )
        .collect())
}

/// Les menus de tete, dans l'ordre, avec leurs entrees de gauche.
pub fn arbre(entrees: &[Entree]) -> Vec<(&Entree, Vec<&Entree>)> {
    let mut tetes: Vec<(&Entree, Vec<&Entree>)> = entrees
        .iter()
        .filter(|e| e.pere == 0 && e.actif)
        .map(|e| (e, Vec::new()))
        .collect();

    for e in entrees.iter().filter(|e| e.pere != 0 && e.actif) {
        if let Some((_, enfants)) = tetes.iter_mut().find(|(t, _)| t.rowid == e.pere) {
            enfants.push(e);
        }
    }
    tetes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(rowid: i64, pere: i64, titre: &str, position: i64, actif: bool) -> Entree {
        Entree {
            rowid,
            pere,
            position,
            titre: titre.to_string(),
            url: String::new(),
            mainmenu: String::new(),
            niveau: if pere == 0 { 0 } else { 1 },
            type_menu: if pere == 0 {
                "top".into()
            } else {
                "left".into()
            },
            actif,
        }
    }

    #[test]
    fn l_arbre_separe_les_tetes_et_les_entrees() {
        let entrees = vec![
            e(1, 0, "Tiers", 10, true),
            e(2, 0, "Produits", 20, true),
            e(3, 1, "Liste", 10, true),
            e(4, 1, "Nouveau", 20, true),
        ];
        let a = arbre(&entrees);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].0.titre, "Tiers");
        assert_eq!(a[0].1.len(), 2);
        assert_eq!(a[1].1.len(), 0);
    }

    #[test]
    fn un_menu_desactive_ne_sort_pas() {
        let entrees = vec![e(1, 0, "Tiers", 10, false), e(2, 0, "Produits", 20, true)];
        let a = arbre(&entrees);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].0.titre, "Produits");
    }

    #[test]
    fn une_entree_orpheline_est_ignoree() {
        // Un enfant dont le pere est absent ne doit pas apparaitre nulle part :
        // mieux vaut un menu incomplet qu'un menu faux.
        let entrees = vec![e(1, 0, "Tiers", 10, true), e(2, 99, "Orpheline", 10, true)];
        let a = arbre(&entrees);
        assert_eq!(a.len(), 1);
        assert!(a[0].1.is_empty());
    }
}
