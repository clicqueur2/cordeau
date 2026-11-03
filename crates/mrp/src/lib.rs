//! MRP : explosion des nomenclatures (BOM).
//!
//! C'est le calcul qui repond a « pour fabriquer N de ceci, il me faut combien
//! de cela ». Dolibarr le fait dans `modMrp` ; voici le coeur, sans ecran :
//! une nomenclature, une quantite, et l'explosion recursive.
//!
//! # Ce que le calcul garantit
//!
//! - **Les quantites se multiplient en decimal exact** (`rust_decimal`), jamais
//!   en flottant : 3 × 0,1 doit valoir 0,3 et pas 0,30000000000000004.
//! - **Un cycle est refuse**, pas contourne. Une nomenclature qui se contient
//!   elle-meme, directement ou par trois niveaux, ferait boucler un serveur
//!   jusqu'a la panne memoire.
//! - **Une profondeur maximale** borne l'explosion : au-dela, on s'arrete et on
//!   le dit, au lieu de rendre un besoin faux sans prevenir.
//! - Un composant qui n'est pas lui-meme une nomenclature est une **feuille** :
//!   c'est une matiere premiere, on la compte et on s'arrete.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

/// Ce qui empeche de calculer.
#[derive(Debug, PartialEq, Eq)]
pub enum Erreur {
    /// Le produit demande n'est pas dans les nomenclatures fournies.
    Inconnu(String),
    /// La nomenclature se contient elle-meme.
    Cycle(Vec<String>),
    /// L'explosion depasse la profondeur permise.
    TropProfond(usize),
    /// Une quantite negative ou nulle n'a pas de sens.
    Quantite(String),
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::Inconnu(p) => write!(f, "produit sans nomenclature : {p}"),
            Erreur::Cycle(chemin) => {
                write!(f, "nomenclature cyclique : {}", chemin.join(" -> "))
            }
            Erreur::TropProfond(n) => write!(f, "explosion plus profonde que {n} niveaux"),
            Erreur::Quantite(q) => write!(f, "quantite invalide : {q}"),
        }
    }
}

impl std::error::Error for Erreur {}

/// Un composant d'une nomenclature : une reference et la quantite par unite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Composant {
    pub reference: String,
    pub quantite: Decimal,
}

/// Une nomenclature : le produit fabrique et ce qu'il consomme.
#[derive(Clone, Debug)]
pub struct Nomenclature {
    pub produit: String,
    pub composants: Vec<Composant>,
}

/// Un besoin net : ce qu'il faut acheter ou prelever.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Besoin {
    pub reference: String,
    pub quantite: Decimal,
    /// Profondeur a laquelle la reference est apparue (1 = sous le produit).
    pub niveau: usize,
}

/// Explose une nomenclature pour une quantite donnee.
///
/// Rend les besoins **agreges par reference** : si une vis apparait dans deux
/// sous-ensembles, elle sort une fois avec la somme.
pub fn exploser(
    nomenclatures: &[Nomenclature],
    produit: &str,
    quantite: Decimal,
    profondeur_max: usize,
) -> Result<Vec<Besoin>, Erreur> {
    if quantite <= Decimal::ZERO {
        return Err(Erreur::Quantite(quantite.to_string()));
    }

    let index: BTreeMap<&str, &Nomenclature> = nomenclatures
        .iter()
        .map(|n| (n.produit.as_str(), n))
        .collect();

    if !index.contains_key(produit) {
        return Err(Erreur::Inconnu(produit.to_string()));
    }

    let mut besoins: BTreeMap<String, Besoin> = BTreeMap::new();
    let mut chemin: Vec<String> = Vec::new();

    // Une pile plutot qu'une recursion : la profondeur est une donnee, pas une
    // limite de la pile d'appels.
    let mut pile: Vec<(String, Decimal, usize)> = vec![(produit.to_string(), quantite, 0)];

    while let Some((courant, qte, niveau)) = pile.pop() {
        let Some(nomenclature) = index.get(courant.as_str()) else {
            // Feuille : matiere premiere.
            let entree = besoins.entry(courant.clone()).or_insert(Besoin {
                reference: courant.clone(),
                quantite: Decimal::ZERO,
                niveau,
            });
            entree.quantite += qte;
            entree.niveau = entree.niveau.min(niveau);
            continue;
        };

        if niveau >= profondeur_max {
            return Err(Erreur::TropProfond(profondeur_max));
        }
        if chemin.contains(&courant) {
            let mut cycle = chemin.clone();
            cycle.push(courant.clone());
            return Err(Erreur::Cycle(cycle));
        }
        chemin.push(courant.clone());

        for composant in &nomenclature.composants {
            if composant.quantite <= Decimal::ZERO {
                return Err(Erreur::Quantite(composant.quantite.to_string()));
            }
            pile.push((
                composant.reference.clone(),
                qte * composant.quantite,
                niveau + 1,
            ));
        }
    }

    Ok(besoins.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn nomen(produit: &str, comp: &[(&str, &str)]) -> Nomenclature {
        Nomenclature {
            produit: produit.to_string(),
            composants: comp
                .iter()
                .map(|(r, q)| Composant {
                    reference: r.to_string(),
                    quantite: d(q),
                })
                .collect(),
        }
    }

    #[test]
    fn un_niveau_multiplie_les_quantites() {
        let n = vec![nomen("TABLE", &[("PLATEAU", "1"), ("PIED", "4")])];
        let b = exploser(&n, "TABLE", d("3"), 5).unwrap();
        assert_eq!(b.len(), 2);
        let pieds = b.iter().find(|x| x.reference == "PIED").unwrap();
        assert_eq!(pieds.quantite, d("12"));
        let plateaux = b.iter().find(|x| x.reference == "PLATEAU").unwrap();
        assert_eq!(plateaux.quantite, d("3"));
    }

    #[test]
    fn deux_niveaux_se_multiplient_en_cascade() {
        let n = vec![
            nomen("MEUBLE", &[("CAISSE", "2"), ("VIS", "8")]),
            nomen("CAISSE", &[("PLANCHE", "3"), ("VIS", "4")]),
        ];
        let b = exploser(&n, "MEUBLE", d("5"), 5).unwrap();
        // 5 meubles = 10 caisses = 30 planches ; vis = 5×8 + 10×4 = 80
        let planches = b.iter().find(|x| x.reference == "PLANCHE").unwrap();
        assert_eq!(planches.quantite, d("30"));
        let vis = b.iter().find(|x| x.reference == "VIS").unwrap();
        assert_eq!(vis.quantite, d("80"));
    }

    #[test]
    fn les_quantites_decimales_restent_exactes() {
        // 3 × 0,1 = 0,3 : en flottant, ce serait 0,30000000000000004.
        let n = vec![nomen("LOT", &[("JOINT", "0.1")])];
        let b = exploser(&n, "LOT", d("3"), 3).unwrap();
        assert_eq!(b[0].quantite, d("0.3"));
        assert_eq!(b[0].quantite.to_string(), "0.3");
    }

    #[test]
    fn un_cycle_est_refuse() {
        let n = vec![nomen("A", &[("B", "1")]), nomen("B", &[("A", "1")])];
        match exploser(&n, "A", d("1"), 10) {
            Err(Erreur::Cycle(_)) => {}
            autre => panic!("cycle attendu, obtenu {autre:?}"),
        }
    }

    #[test]
    fn une_profondeur_trop_grande_est_refusee() {
        let n = vec![
            nomen("A", &[("B", "1")]),
            nomen("B", &[("C", "1")]),
            nomen("C", &[("D", "1")]),
        ];
        assert_eq!(exploser(&n, "A", d("1"), 2), Err(Erreur::TropProfond(2)));
        assert!(exploser(&n, "A", d("1"), 3).is_ok());
    }

    #[test]
    fn une_quantite_nulle_ou_inconnue_est_refusee() {
        let n = vec![nomen("A", &[("B", "1")])];
        assert!(matches!(
            exploser(&n, "A", Decimal::ZERO, 3),
            Err(Erreur::Quantite(_))
        ));
        assert!(matches!(
            exploser(&n, "Z", d("1"), 3),
            Err(Erreur::Inconnu(_))
        ));
    }
}
