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
    //
    // Chaque noeud ouvert pose une marque `Fermer` **avant** ses composants :
    // en depilant, les composants passent d'abord (LIFO), puis la marque retire
    // le noeud du chemin. Sans cela, le chemin gardait toutes les branches
    // visitees et la meme piece presente dans deux sous-ensembles — cas
    // ordinaire d'une nomenclature — etait declaree « cycle ».
    enum Tache {
        Ouvrir(String, Decimal, usize),
        Fermer,
    }
    let mut pile: Vec<Tache> = vec![Tache::Ouvrir(produit.to_string(), quantite, 0)];

    while let Some(tache) = pile.pop() {
        let (courant, qte, niveau) = match tache {
            Tache::Fermer => {
                // Fin de la branche : on remonte d'un cran.
                chemin.pop();
                continue;
            }
            Tache::Ouvrir(courant, qte, niveau) => (courant, qte, niveau),
        };

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
        pile.push(Tache::Fermer);

        for composant in &nomenclature.composants {
            if composant.quantite <= Decimal::ZERO {
                return Err(Erreur::Quantite(composant.quantite.to_string()));
            }
            pile.push(Tache::Ouvrir(
                composant.reference.clone(),
                qte * composant.quantite,
                niveau + 1,
            ));
        }
    }

    Ok(besoins.into_values().collect())
}

// ---------------------------------------------------------------------------
// Besoin net et ordre propose
// ---------------------------------------------------------------------------

/// Ce qu'on sait d'un article sur un horizon donne.
///
/// Les quantites sont decimales : un metre de cable, 0,5 kg de resine. Un
/// arrondi au lot se fait **une fois**, a la fin, jamais en chemin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Article {
    pub reference: String,
    /// Ce que la demande exige (commandes clients a livrer).
    pub besoin_brut: Decimal,
    /// Ce qui est en stock.
    pub stock: Decimal,
    /// Ce qui doit arriver (commandes fournisseurs, ordres de fabrication).
    pub entrees: Decimal,
    /// Multiple d'achat ou de fabrication. Zero = pas d'arrondi.
    pub lot: Decimal,
    /// Quantite minimale d'un ordre. Zero = pas de minimum.
    pub minimum: Decimal,
    /// Delai d'approvisionnement en jours.
    pub delai_jours: i64,
}

/// Ce que le calcul propose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposition {
    pub reference: String,
    pub besoin_brut: Decimal,
    pub stock: Decimal,
    pub entrees: Decimal,
    /// Besoin brut moins stock et entrees, jamais negatif.
    pub besoin_net: Decimal,
    /// La quantite a commander, arrondie au lot et au minimum.
    pub quantite: Decimal,
    /// La date a laquelle lancer, compte tenu du delai.
    pub date_lancement: String,
    /// La date de lancement est-elle deja passee ?
    pub en_retard: bool,
}

/// Arrondit une quantite au multiple et au minimum.
///
/// L'ordre compte : on arrondit **au lot**, puis on remonte au minimum, puis
/// **on re-arrondit au lot** — sinon un minimum de 10 avec un lot de 12
/// donnerait 10, qui n'est pas un multiple.
pub fn arrondir(quantite: Decimal, lot: Decimal, minimum: Decimal) -> Decimal {
    if quantite <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let mut q = quantite;
    if lot > Decimal::ZERO {
        q = (q / lot).ceil() * lot;
    }
    if minimum > Decimal::ZERO && q < minimum {
        q = minimum;
        if lot > Decimal::ZERO {
            q = (q / lot).ceil() * lot;
        }
    }
    q
}

/// Calcule le besoin net et l'ordre propose d'un article.
///
/// `aujourd_hui` sert uniquement a dire si la date de lancement est deja
/// passee. Une date de lancement dans le passe n'est pas une erreur de calcul :
/// c'est un retard, et il faut le voir.
pub fn proposer(article: &Article, date_besoin: &str, aujourd_hui: &str) -> Proposition {
    let couverture = article.stock + article.entrees;
    let besoin_net = (article.besoin_brut - couverture).max(Decimal::ZERO);
    let quantite = arrondir(besoin_net, article.lot, article.minimum);
    let date_lancement = recule(date_besoin, article.delai_jours);

    Proposition {
        reference: article.reference.clone(),
        besoin_brut: article.besoin_brut,
        stock: article.stock,
        entrees: article.entrees,
        besoin_net,
        quantite,
        en_retard: date_lancement.as_str() < aujourd_hui,
        date_lancement,
    }
}

/// Recule une date `AAAA-MM-JJ` d'un nombre de jours.
///
/// Les jours sont comptes en jours juliens : ni fuseau, ni heure d'ete, ni
/// dependance de calendrier. Une date illisible est rendue telle quelle plutot
/// que remplacee par une valeur inventee.
pub fn recule(date: &str, jours: i64) -> String {
    let Some(j) = jour_julien(date) else {
        return date.to_string();
    };
    depuis_julien(j - jours)
}

/// Le jour julien d'une date `AAAA-MM-JJ`, ou `None` si elle est illisible.
fn jour_julien(date: &str) -> Option<i64> {
    let mut parties = date.split('-');
    let annee: i64 = parties.next()?.parse().ok()?;
    let mois: i64 = parties.next()?.parse().ok()?;
    let jour: i64 = parties.next()?.parse().ok()?;
    if !(1..=12).contains(&mois) || !(1..=31).contains(&jour) {
        return None;
    }
    // Algorithme de Fliegel-Van Flandern.
    let a = (14 - mois) / 12;
    let y = annee + 4800 - a;
    let m = mois + 12 * a - 3;
    Some(jour + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045)
}

/// La date `AAAA-MM-JJ` d'un jour julien.
fn depuis_julien(j: i64) -> String {
    let a = j + 32044;
    let b = (4 * a + 3) / 146_097;
    let c = a - (146_097 * b) / 4;
    let d = (4 * c + 3) / 1461;
    let e = c - (1461 * d) / 4;
    let m = (5 * e + 2) / 153;
    let jour = e - (153 * m + 2) / 5 + 1;
    let mois = m + 3 - 12 * (m / 10);
    let annee = 100 * b + d - 4800 + m / 10;
    format!("{annee:04}-{mois:02}-{jour:02}")
}

#[cfg(test)]
mod tests_besoin {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn article(brut: &str, stock: &str, entrees: &str) -> Article {
        Article {
            reference: "REF".into(),
            besoin_brut: d(brut),
            stock: d(stock),
            entrees: d(entrees),
            lot: Decimal::ZERO,
            minimum: Decimal::ZERO,
            delai_jours: 0,
        }
    }

    #[test]
    fn le_stock_reduit_le_besoin() {
        let p = proposer(&article("100", "30", "0"), "2026-12-31", "2026-11-04");
        assert_eq!(p.besoin_net, d("70"));
        assert_eq!(p.quantite, d("70"));
    }

    #[test]
    fn les_entrees_attendues_reduisent_aussi() {
        let p = proposer(&article("100", "30", "20"), "2026-12-31", "2026-11-04");
        assert_eq!(p.besoin_net, d("50"));
    }

    #[test]
    fn un_stock_suffisant_ne_propose_rien() {
        let p = proposer(&article("10", "50", "0"), "2026-12-31", "2026-11-04");
        assert_eq!(p.besoin_net, Decimal::ZERO);
        assert_eq!(p.quantite, Decimal::ZERO);
    }

    #[test]
    fn l_arrondi_au_lot_monte_et_ne_descend_jamais() {
        assert_eq!(arrondir(d("70"), d("25"), Decimal::ZERO), d("75"));
        assert_eq!(arrondir(d("50"), d("25"), Decimal::ZERO), d("50"));
        assert_eq!(arrondir(d("1"), d("25"), Decimal::ZERO), d("25"));
        // Pas de lot : la quantite reste telle quelle.
        assert_eq!(arrondir(d("7.5"), Decimal::ZERO, Decimal::ZERO), d("7.5"));
    }

    #[test]
    fn le_minimum_se_combine_avec_le_lot() {
        // Minimum 10, lot 12 : 10 n'est pas un multiple, on monte a 12.
        assert_eq!(arrondir(d("3"), d("12"), d("10")), d("12"));
        // Minimum 10, lot 0 : on prend le minimum.
        assert_eq!(arrondir(d("3"), Decimal::ZERO, d("10")), d("10"));
        // Deja au-dessus du minimum : le minimum ne joue pas.
        assert_eq!(arrondir(d("30"), d("12"), d("10")), d("36"));
    }

    #[test]
    fn le_delai_recule_la_date_de_lancement() {
        let mut a = article("10", "0", "0");
        a.delai_jours = 15;
        let p = proposer(&a, "2026-12-31", "2026-11-04");
        assert_eq!(p.date_lancement, "2026-12-16");
        assert!(!p.en_retard);
    }

    #[test]
    fn une_date_de_lancement_passee_est_un_retard() {
        let mut a = article("10", "0", "0");
        a.delai_jours = 15;
        let p = proposer(&a, "2026-11-10", "2026-11-04");
        assert_eq!(p.date_lancement, "2026-10-26");
        assert!(p.en_retard, "le 26 octobre est passe");
    }

    #[test]
    fn les_dates_traversent_les_mois_et_les_annees() {
        assert_eq!(recule("2026-03-01", 1), "2026-02-28");
        assert_eq!(recule("2026-01-01", 1), "2025-12-31");
        assert_eq!(recule("2026-11-04", 0), "2026-11-04");
        assert_eq!(recule("2026-11-04", 365), "2025-11-04");
        // Une annee bissextile compte 29 fevrier.
        assert_eq!(recule("2024-03-01", 1), "2024-02-29");
    }

    #[test]
    fn une_date_illisible_est_rendue_telle_quelle() {
        assert_eq!(recule("pas une date", 5), "pas une date");
        assert_eq!(recule("2026-13-01", 5), "2026-13-01");
    }
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

    /// Nomenclature en diamant : A contient B et C, B et C contiennent la meme
    /// vis. Ce n'est pas un cycle, et la vis doit sortir une fois, sommee.
    ///
    /// Le chemin n'etait jamais depile : la seconde branche retrouvait la vis
    /// deja visitee et l'explosion etait refusee avec « Cycle ».
    #[test]
    fn une_piece_partagee_n_est_pas_un_cycle() {
        use rust_decimal::Decimal;
        use std::str::FromStr;

        let d = |s: &str| Decimal::from_str(s).unwrap();
        let nom = |produit: &str, composants: &[(&str, &str)]| Nomenclature {
            produit: produit.to_string(),
            composants: composants
                .iter()
                .map(|(r, q)| Composant {
                    reference: r.to_string(),
                    quantite: d(q),
                })
                .collect(),
        };

        let nomenclatures = vec![
            nom("A", &[("B", "1"), ("C", "1")]),
            nom("B", &[("vis", "2")]),
            nom("C", &[("vis", "3")]),
        ];

        let besoins = exploser(&nomenclatures, "A", d("1"), 10).expect("pas un cycle");
        let vis = besoins
            .iter()
            .find(|b| b.reference == "vis")
            .expect("la vis doit sortir");
        assert_eq!(vis.quantite, d("5"), "2 + 3");
        assert_eq!(besoins.len(), 1, "une seule reference, agregee");
    }
}
