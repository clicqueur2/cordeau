//! Moteur paie : application d'un bareme de cotisations a un brut.
//!
//! # Ce crate ne contient aucun droit social
//!
//! Il n'y a ici **aucun taux, aucun plafond, aucune cotisation** : pas de CSG,
//! pas de vieillesse, pas de mutuelle, pas de PASS. Ce crate est une **machine a
//! calculer vide**. Le bareme arrive en argument, il est applique tel quel, et
//! chaque ligne produite porte sa base et son taux pour qu'un comptable puisse
//! refaire le calcul a la main.
//!
//! La raison est simple : le droit social francais change chaque annee, et un
//! taux invente dans du code est un faux bulletin. Le bareme doit venir d'une
//! source officielle (URSSAF, Net-entreprises, convention collective) et etre
//! valide contre des bulletins reels avant de servir.
//!
//! # Ce que la mecanique garantit
//!
//! - **Trois assiettes seulement** : le brut, le brut borne au plafond mensuel
//!   (`BrutPlafonne`), et le net courant (`Net`). Une cotisation sur le net se
//!   calcule sur ce qui reste apres les lignes precedentes de la liste, donc
//!   **l'ordre de la liste fait partie du resultat** et il est respecte tel quel.
//! - **Un plafond optionnel par cotisation** borne encore la base
//!   (`base = min(base, plafond)`), apres le plafond mensuel. Les deux bornes
//!   sont des minimums, donc l'ordre entre elles est sans effet.
//! - **Arrondi commercial au centime, ligne par ligne** (demi hors de zero, comme
//!   `money::round_money`). Le total est la **somme des lignes arrondies**,
//!   jamais le recalcul du total : sinon le bulletin ne s'additionne pas.
//! - **Le bulletin s'additionne exactement** : `brut = total_cotisations +
//!   net_a_payer`, verifie sur chaque calcul. Le net n'est pas re-arrondi apres
//!   coup (un arrondi supplementaire casserait l'egalite au centime).
//! - **Aucun flottant** : tout est en `Decimal` (`rust_decimal`).
//! - **Refus explicites** plutot que resultat faux : un bareme vide, un taux
//!   negatif, un brut nul ou negatif, un net qui devient negatif, un plafond
//!   negatif, ou un plafond mensuel non positif alors qu'il sert, arretent le
//!   calcul. Un bulletin faux vaut moins qu'une erreur.

use std::fmt;

use rust_decimal::{Decimal, RoundingStrategy};

/// Nombre de decimales d'un montant en euros.
const SCALE_CENTIME: u32 = 2;

/// Arrondi commercial au centime : demi hors de zero.
///
/// C'est l'arrondi de `money::round_money`, donc celui de Dolibarr
/// (`price2num($x, 'MT')`) : 0,125 devient 0,13 et non 0,12. L'arrondi bancaire
/// donnerait 0,12 et ne correspondrait pas a ce qu'attend un comptable.
pub fn arrondi_centime(montant: Decimal) -> Decimal {
    montant.round_dp_with_strategy(SCALE_CENTIME, RoundingStrategy::MidpointAwayFromZero)
}

/// Sur quoi la cotisation se calcule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assiette {
    /// Le brut, sans borne autre que le plafond propre a la cotisation.
    Brut,
    /// Le brut borne au plafond mensuel fourni a `calculer`.
    BrutPlafonne,
    /// Le net courant : le brut moins les cotisations deja calculees.
    Net,
}

impl Assiette {
    /// Code stable de l'assiette, ecrit sans accent : `brut`, `brut_plafonne`,
    /// `net`. Sert a un chargeur de bareme (fichier JSON ou YAML ecrit hors de
    /// ce crate) qui doit relire ce qu'il a produit.
    pub fn code(&self) -> &'static str {
        match self {
            Assiette::Brut => "brut",
            Assiette::BrutPlafonne => "brut_plafonne",
            Assiette::Net => "net",
        }
    }

    /// Inverse de [`Assiette::code`] : traduit un code de bareme en assiette.
    ///
    /// Tolere les espaces autour et la casse. Tout le reste est refuse avec
    /// [`Erreur::AssietteInconnue`] : on ne devine pas une assiette, une
    /// cotisation sur la mauvaise base est un faux bulletin.
    ///
    /// Avec les types de ce crate, une assiette inconnue est impossible a
    /// construire ; cette fonction existe pour la frontiere (fichier, table,
    /// import) ou l'assiette arrive encore sous forme de texte.
    pub fn depuis_code(code: &str) -> Result<Assiette, Erreur> {
        let code = code.trim();
        if code.eq_ignore_ascii_case("brut") {
            Ok(Assiette::Brut)
        } else if code.eq_ignore_ascii_case("brut_plafonne") {
            Ok(Assiette::BrutPlafonne)
        } else if code.eq_ignore_ascii_case("net") {
            Ok(Assiette::Net)
        } else {
            Err(Erreur::AssietteInconnue(code.to_string()))
        }
    }
}

/// Une cotisation du bareme : quoi, sur quoi, a quel taux, et jusqu'a quel
/// plafond.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cotisation {
    /// Code de la cotisation, tel qu'il figurera sur la ligne (ex. code interne
    /// du bareme ; ce crate ne fixe aucune nomenclature).
    pub code: String,
    /// Libelle imprime sur le bulletin.
    pub libelle: String,
    /// Assiette : `brut`, `brut_plafonne` ou `net`.
    pub assiette: Assiette,
    /// Taux en **pourcentage** : 21,50 vaut 21,50 %, pas 0,215.
    pub taux: Decimal,
    /// Plafond propre a la cotisation, en euros, applique a la base.
    pub plafond: Option<Decimal>,
}

/// Un bareme de cotisations : un libelle, une date d'effet, des lignes.
///
/// Le contenu vient de l'appelant. Ce crate ne fournit ni barème par defaut ni
/// barème d'exemple chiffre, volontairement : un bareme en dur dans le code
/// serait pris pour la verite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bareme {
    /// Libelle du bareme (ex. « grille interne 2026 »).
    pub libelle: String,
    /// Date d'effet, au format `AAAA-MM-JJ`, conservee telle quelle.
    pub effet_le: String,
    /// Les cotisations, dans l'ordre d'application.
    pub cotisations: Vec<Cotisation>,
}

/// Une ligne du bulletin : la trace du calcul, rejouable a la main.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LigneCotisation {
    /// Code de la cotisation.
    pub code: String,
    /// Libelle imprime.
    pub libelle: String,
    /// Base retenue apres application du plafond mensuel et du plafond de ligne.
    pub base: Decimal,
    /// Taux applique, en pourcentage.
    pub taux: Decimal,
    /// Montant arrondi au centime.
    pub montant: Decimal,
}

/// Le resultat d'un calcul, coherent par construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bulletin {
    /// Brut de depart, repris tel quel.
    pub brut: Decimal,
    /// Les lignes, dans l'ordre du bareme.
    pub lignes: Vec<LigneCotisation>,
    /// Somme des montants arrondis des lignes.
    pub total_cotisations: Decimal,
    /// `brut - total_cotisations`, sans arrondi supplementaire.
    pub net_a_payer: Decimal,
}

/// Ce qui empeche de calculer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Erreur {
    /// Le bareme ne contient aucune cotisation : il n'y a rien a appliquer.
    BaremeVide,
    /// Un taux est negatif, ou une autre valeur numerique d'une cotisation est
    /// inutilisable (plafond de ligne negatif, plafond mensuel non positif
    /// alors qu'une cotisation plafonnee l'utilise). Le message nomme la
    /// cotisation et la valeur fautive.
    TauxInvalide(String),
    /// Le brut est nul ou negatif.
    BrutInvalide,
    /// Les cotisations depassent le brut : le bareme ou le brut est incoherent.
    NetNegatif,
    /// L'assiette demandee n'existe pas (voir [`Assiette::depuis_code`]).
    AssietteInconnue(String),
}

impl fmt::Display for Erreur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erreur::BaremeVide => write!(f, "bareme vide : aucune cotisation a appliquer"),
            Erreur::TauxInvalide(detail) => write!(f, "valeur de bareme invalide : {detail}"),
            Erreur::BrutInvalide => write!(f, "brut invalide : il doit etre strictement positif"),
            Erreur::NetNegatif => write!(
                f,
                "net negatif : les cotisations depassent le brut, le bareme est incoherent"
            ),
            Erreur::AssietteInconnue(code) => write!(
                f,
                "assiette inconnue : « {code} » (attendu : brut, brut_plafonne, net)"
            ),
        }
    }
}

impl std::error::Error for Erreur {}

/// Verifie le bareme **avant** de calculer quoi que ce soit.
///
/// Refuser tot, c'est refuser avant d'avoir produit une seule ligne : aucun
/// bulletin partiel ne peut sortir d'un bareme douteux.
fn valider(cotisations: &[Cotisation], plafond_mensuel: Decimal) -> Result<(), Erreur> {
    // Le plafond mensuel n'est exige que s'il sert reellement : sans cotisation
    // plafonnee, la valeur passee par l'appelant n'a aucun effet.
    let plafond_mensuel_utile = cotisations
        .iter()
        .any(|c| c.assiette == Assiette::BrutPlafonne);
    if plafond_mensuel_utile && plafond_mensuel <= Decimal::ZERO {
        return Err(Erreur::TauxInvalide(format!(
            "plafond mensuel non positif ({plafond_mensuel}) alors qu'une cotisation est plafonnee"
        )));
    }

    for cotisation in cotisations {
        if cotisation.taux < Decimal::ZERO {
            return Err(Erreur::TauxInvalide(format!(
                "{} : taux negatif ({})",
                cotisation.code, cotisation.taux
            )));
        }
        // Un plafond de ligne nul est accepte : base nulle, montant nul, c'est
        // arithmetiquement coherent et parfois voulu. Un plafond negatif
        // donnerait une base negative, donc un montant negatif qui augmenterait
        // le net : c'est refuse.
        if let Some(plafond) = cotisation.plafond {
            if plafond < Decimal::ZERO {
                return Err(Erreur::TauxInvalide(format!(
                    "{} : plafond negatif ({plafond})",
                    cotisation.code
                )));
            }
        }
    }

    Ok(())
}

/// Applique un bareme a un brut.
///
/// `plafond_mensuel` est le plafond auquel borne l'assiette `BrutPlafonne` ; il
/// n'a d'effet que sur les cotisations qui declarent cette assiette.
///
/// Le calcul parcourt les cotisations **dans l'ordre de la liste**, en tenant a
/// jour le total arrondi : une cotisation d'assiette `Net` voit donc le net
/// laisse par les lignes precedentes.
pub fn calculer(
    bareme: &Bareme,
    brut: Decimal,
    plafond_mensuel: Decimal,
) -> Result<Bulletin, Erreur> {
    if bareme.cotisations.is_empty() {
        return Err(Erreur::BaremeVide);
    }
    if brut <= Decimal::ZERO {
        return Err(Erreur::BrutInvalide);
    }
    valider(&bareme.cotisations, plafond_mensuel)?;

    let cent = Decimal::new(100, 0);
    let mut lignes: Vec<LigneCotisation> = Vec::with_capacity(bareme.cotisations.len());
    // Somme des montants DEJA arrondis : c'est le total du bulletin.
    let mut total_cotisations = Decimal::ZERO;

    for cotisation in &bareme.cotisations {
        // Assiette, puis plafond de ligne : deux bornes hautes, appliquees dans
        // cet ordre. `min` etant associatif, l'ordre entre les deux est sans
        // effet sur le resultat.
        let mut base = match cotisation.assiette {
            Assiette::Brut => brut,
            Assiette::BrutPlafonne => brut.min(plafond_mensuel),
            // Net courant au moment de la ligne : le brut moins les lignes deja
            // comptees. Sans re-arrondi, puisque chaque montant est deja rond.
            Assiette::Net => brut - total_cotisations,
        };
        if let Some(plafond) = cotisation.plafond {
            base = base.min(plafond);
        }

        let montant = arrondi_centime(base * cotisation.taux / cent);
        total_cotisations += montant;

        lignes.push(LigneCotisation {
            code: cotisation.code.clone(),
            libelle: cotisation.libelle.clone(),
            base,
            taux: cotisation.taux,
            montant,
        });
    }

    // Le net n'est pas re-arrondi : chaque montant etant deja au centime, la
    // soustraction reste au centime et l'egalite brut = total + net tient
    // exactement. Si le brut porte plus de deux decimales, le net en porte
    // autant : mieux vaut un bulletin qui s'additionne qu'un centime invente.
    let net_a_payer = brut - total_cotisations;
    if net_a_payer < Decimal::ZERO {
        return Err(Erreur::NetNegatif);
    }

    Ok(Bulletin {
        brut,
        lignes,
        total_cotisations,
        net_a_payer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn bareme(cotisations: Vec<Cotisation>) -> Bareme {
        Bareme {
            libelle: "bareme de test (fictif, aucun taux reel)".to_string(),
            effet_le: "2026-01-01".to_string(),
            cotisations,
        }
    }

    fn cotisation(code: &str, assiette: Assiette, taux: &str) -> Cotisation {
        Cotisation {
            code: code.to_string(),
            libelle: format!("cotisation {code}"),
            assiette,
            taux: d(taux),
            plafond: None,
        }
    }

    fn cotisation_plafonnee(
        code: &str,
        assiette: Assiette,
        taux: &str,
        plafond: &str,
    ) -> Cotisation {
        Cotisation {
            plafond: Some(d(plafond)),
            ..cotisation(code, assiette, taux)
        }
    }

    #[test]
    fn tracabilite_brut_plafonne_21_50() {
        // Le cas de controle : 2 500 a 21,50 % sur une assiette plafonnee a
        // 3 000 donne une base de 2 500 et 537,50. Le plafond de 3 000 ne mord
        // pas. La ligne doit porter la base et le taux pour etre rejouable.
        let b = bareme(vec![cotisation("TEST", Assiette::BrutPlafonne, "21.50")]);
        let bulletin = calculer(&b, d("2500"), d("3000")).unwrap();

        assert_eq!(bulletin.brut, d("2500"));
        assert_eq!(bulletin.lignes.len(), 1);

        let ligne = &bulletin.lignes[0];
        assert_eq!(ligne.code, "TEST");
        assert_eq!(ligne.libelle, "cotisation TEST");
        assert_eq!(ligne.base, d("2500"));
        assert_eq!(ligne.taux, d("21.50"));
        assert_eq!(ligne.montant, d("537.50"));
        // Exactement deux decimales : aucun residu de flottant.
        assert!(ligne.montant.scale() <= SCALE_CENTIME);

        assert_eq!(bulletin.total_cotisations, d("537.50"));
        assert_eq!(bulletin.net_a_payer, d("1962.50"));
        assert_eq!(
            bulletin.total_cotisations + bulletin.net_a_payer,
            bulletin.brut
        );
    }

    #[test]
    fn brut_plafonne_borne_au_plafond_mensuel() {
        let b = bareme(vec![cotisation("TEST", Assiette::BrutPlafonne, "21.50")]);
        let bulletin = calculer(&b, d("3500"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].base, d("3000"));
        assert_eq!(bulletin.lignes[0].montant, d("645.00"));
        assert_eq!(bulletin.total_cotisations, d("645.00"));
        assert_eq!(bulletin.net_a_payer, d("2855.00"));
    }

    #[test]
    fn assiette_brut_ignore_le_plafond_mensuel() {
        let b = bareme(vec![cotisation("TEST", Assiette::Brut, "10")]);
        let bulletin = calculer(&b, d("3500"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].base, d("3500"));
        assert_eq!(bulletin.lignes[0].montant, d("350.00"));
    }

    #[test]
    fn assiette_net_se_calcule_sur_le_net_courant() {
        // 2 000 : 10 % sur le brut = 200,00 ; puis 5 % sur le net restant
        // (1 800,00) = 90,00 ; puis 2 % sur 1 710,00 = 34,20.
        let b = bareme(vec![
            cotisation("A", Assiette::Brut, "10"),
            cotisation("B", Assiette::Net, "5"),
            cotisation("C", Assiette::Net, "2"),
        ]);
        let bulletin = calculer(&b, d("2000"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].base, d("2000"));
        assert_eq!(bulletin.lignes[1].base, d("1800"));
        assert_eq!(bulletin.lignes[2].base, d("1710"));

        assert_eq!(bulletin.lignes[0].montant, d("200.00"));
        assert_eq!(bulletin.lignes[1].montant, d("90.00"));
        assert_eq!(bulletin.lignes[2].montant, d("34.20"));

        assert_eq!(bulletin.total_cotisations, d("324.20"));
        assert_eq!(bulletin.net_a_payer, d("1675.80"));
    }

    #[test]
    fn l_ordre_de_la_liste_fait_partie_du_resultat() {
        // Memes cotisations, deux ordres : la ligne au net ne voit pas le meme
        // net, donc le total differe. L'ordre de la liste est respecte tel quel.
        let net_puis_brut = bareme(vec![
            cotisation("NET", Assiette::Net, "10"),
            cotisation("BRUT", Assiette::Brut, "20"),
        ]);
        let brut_puis_net = bareme(vec![
            cotisation("BRUT", Assiette::Brut, "20"),
            cotisation("NET", Assiette::Net, "10"),
        ]);

        let a = calculer(&net_puis_brut, d("1000"), d("3000")).unwrap();
        let b = calculer(&brut_puis_net, d("1000"), d("3000")).unwrap();

        // Net en premier : la base nette vaut le brut entier (1 000).
        assert_eq!(a.lignes[0].base, d("1000"));
        assert_eq!(a.total_cotisations, d("300.00"));
        // Brut en premier : la base nette vaut 800.
        assert_eq!(b.lignes[0].base, d("1000"));
        assert_eq!(b.lignes[1].base, d("800"));
        assert_eq!(b.total_cotisations, d("280.00"));
    }

    #[test]
    fn plafond_de_cotisation_borne_la_base() {
        let b = bareme(vec![cotisation_plafonnee("X", Assiette::Brut, "5", "1000")]);
        let bulletin = calculer(&b, d("3000"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].base, d("1000"));
        assert_eq!(bulletin.lignes[0].montant, d("50.00"));
    }

    #[test]
    fn plafond_de_cotisation_au_dessus_de_la_base_ne_change_rien() {
        let b = bareme(vec![cotisation_plafonnee("X", Assiette::Brut, "5", "5000")]);
        let bulletin = calculer(&b, d("3000"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].base, d("3000"));
        assert_eq!(bulletin.lignes[0].montant, d("150.00"));
    }

    #[test]
    fn plafond_mensuel_et_plafond_de_cotisation_se_cumulent_par_le_minimum() {
        // Brut 4 000, plafond mensuel 3 000, plafond de ligne 2 000 : la base
        // la plus basse gagne, soit 2 000.
        let b = bareme(vec![cotisation_plafonnee(
            "X",
            Assiette::BrutPlafonne,
            "10",
            "2000",
        )]);
        let bulletin = calculer(&b, d("4000"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].base, d("2000"));
        assert_eq!(bulletin.lignes[0].montant, d("200.00"));
    }

    #[test]
    fn plafond_de_cotisation_nul_donne_une_base_nulle() {
        // Un plafond de ligne a 0 est arithmetiquement coherent : base 0,
        // montant 0. C'est le plafond negatif qui est refuse.
        let b = bareme(vec![cotisation_plafonnee("X", Assiette::Brut, "10", "0")]);
        let bulletin = calculer(&b, d("2000"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].base, Decimal::ZERO);
        assert_eq!(bulletin.lignes[0].montant, Decimal::ZERO);
        assert_eq!(bulletin.net_a_payer, d("2000"));
    }

    #[test]
    fn arrondi_commercial_au_centime_par_ligne() {
        // 12,50 x 1 % = 0,125 -> 0,13 (demi hors de zero, pas 0,12).
        let b = bareme(vec![cotisation("X", Assiette::Brut, "1")]);
        let bulletin = calculer(&b, d("12.50"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].montant, d("0.13"));
        assert_eq!(bulletin.net_a_payer, d("12.37"));
        assert_eq!(arrondi_centime(d("0.125")), d("0.13"));
        assert_eq!(arrondi_centime(d("1.005")), d("1.01"));
        assert_eq!(arrondi_centime(d("1.004")), d("1.00"));
    }

    #[test]
    fn total_est_la_somme_des_lignes_arrondies() {
        // 100 x 33,3333 % = 33,3333 -> 33,33 par ligne. La somme des lignes
        // vaut 99,99 ; le recalcul du total donnerait 100,00 et le bulletin ne
        // s'additionnerait plus. On garde 99,99.
        let b = bareme(vec![
            cotisation("A", Assiette::Brut, "33.3333"),
            cotisation("B", Assiette::Brut, "33.3333"),
            cotisation("C", Assiette::Brut, "33.3333"),
        ]);
        let bulletin = calculer(&b, d("100"), d("3000")).unwrap();

        for ligne in &bulletin.lignes {
            assert_eq!(ligne.montant, d("33.33"));
        }
        assert_eq!(bulletin.total_cotisations, d("99.99"));
        assert_eq!(bulletin.net_a_payer, d("0.01"));
    }

    #[test]
    fn taux_nul_est_accepte_et_produit_zero() {
        // La borne du refus : 0 est un taux valide (montant nul), seul le
        // negatif est refuse.
        let b = bareme(vec![cotisation("X", Assiette::Brut, "0")]);
        let bulletin = calculer(&b, d("2000"), d("3000")).unwrap();

        assert_eq!(bulletin.lignes[0].base, d("2000"));
        assert_eq!(bulletin.lignes[0].montant, Decimal::ZERO);
        assert_eq!(bulletin.net_a_payer, d("2000"));
    }

    #[test]
    fn bareme_vide_est_refuse() {
        let b = bareme(vec![]);
        assert_eq!(calculer(&b, d("2000"), d("3000")), Err(Erreur::BaremeVide));
    }

    #[test]
    fn taux_negatif_est_refuse() {
        let b = bareme(vec![cotisation("X", Assiette::Brut, "-1.5")]);
        match calculer(&b, d("2000"), d("3000")) {
            Err(Erreur::TauxInvalide(detail)) => {
                assert!(detail.contains('X'), "message sans le code : {detail}");
                assert!(
                    detail.contains("taux"),
                    "message sans le mot taux : {detail}"
                );
            }
            autre => panic!("taux negatif attendu refuse, obtenu {autre:?}"),
        }
    }

    #[test]
    fn brut_nul_ou_negatif_est_refuse() {
        let b = bareme(vec![cotisation("X", Assiette::Brut, "10")]);
        assert_eq!(
            calculer(&b, Decimal::ZERO, d("3000")),
            Err(Erreur::BrutInvalide)
        );
        assert_eq!(
            calculer(&b, d("-2000"), d("3000")),
            Err(Erreur::BrutInvalide)
        );
    }

    #[test]
    fn net_negatif_est_refuse() {
        // 100 a 150 % : il reste -50, le bareme est incoherent.
        let b = bareme(vec![cotisation("X", Assiette::Brut, "150")]);
        assert_eq!(calculer(&b, d("100"), d("3000")), Err(Erreur::NetNegatif));
    }

    #[test]
    fn plafond_de_cotisation_negatif_est_refuse() {
        let b = bareme(vec![cotisation_plafonnee("X", Assiette::Brut, "10", "-1")]);
        assert!(matches!(
            calculer(&b, d("2000"), d("3000")),
            Err(Erreur::TauxInvalide(_))
        ));
    }

    #[test]
    fn plafond_mensuel_non_positif_est_refuse_quand_il_sert() {
        let b = bareme(vec![cotisation("X", Assiette::BrutPlafonne, "10")]);
        assert!(matches!(
            calculer(&b, d("2000"), Decimal::ZERO),
            Err(Erreur::TauxInvalide(_))
        ));
        assert!(matches!(
            calculer(&b, d("2000"), d("-3000")),
            Err(Erreur::TauxInvalide(_))
        ));
        // Avec un plafond positif, le meme bareme passe.
        assert!(calculer(&b, d("2000"), d("3000")).is_ok());
    }

    #[test]
    fn plafond_mensuel_ignore_quand_aucune_cotisation_n_est_plafonnee() {
        // Aucune cotisation n'utilise le plafond mensuel : sa valeur n'a aucune
        // incidence, une valeur nulle ne doit donc pas faire echouer le calcul.
        let b = bareme(vec![cotisation("X", Assiette::Brut, "10")]);
        let bulletin = calculer(&b, d("2000"), Decimal::ZERO).unwrap();

        assert_eq!(bulletin.lignes[0].base, d("2000"));
        assert_eq!(bulletin.total_cotisations, d("200.00"));
    }

    #[test]
    fn codes_d_assiette_font_l_aller_retour() {
        assert_eq!(Assiette::depuis_code("brut").unwrap(), Assiette::Brut);
        assert_eq!(
            Assiette::depuis_code("  BRUT_PLAFONNE ").unwrap(),
            Assiette::BrutPlafonne
        );
        assert_eq!(Assiette::depuis_code("Net").unwrap(), Assiette::Net);
        for assiette in [Assiette::Brut, Assiette::BrutPlafonne, Assiette::Net] {
            assert_eq!(
                Assiette::depuis_code(assiette.code()).unwrap(),
                assiette,
                "aller-retour casse pour {}",
                assiette.code()
            );
        }
    }

    #[test]
    fn assiette_inconnue_est_refusee() {
        assert_eq!(
            Assiette::depuis_code("prime"),
            Err(Erreur::AssietteInconnue("prime".to_string()))
        );
        // Un code approchant ne doit pas etre devine.
        assert!(matches!(
            Assiette::depuis_code("brut plafonne"),
            Err(Erreur::AssietteInconnue(_))
        ));
        assert!(matches!(
            Assiette::depuis_code(""),
            Err(Erreur::AssietteInconnue(_))
        ));
    }

    #[test]
    fn erreur_est_une_erreur_standard() {
        let boite: Box<dyn std::error::Error> = Box::new(Erreur::BaremeVide);
        assert!(!boite.to_string().is_empty());

        for erreur in [
            Erreur::BaremeVide,
            Erreur::TauxInvalide("X : taux negatif".to_string()),
            Erreur::BrutInvalide,
            Erreur::NetNegatif,
            Erreur::AssietteInconnue("prime".to_string()),
        ] {
            assert!(
                !erreur.to_string().is_empty(),
                "message vide pour {erreur:?}"
            );
        }
    }

    #[test]
    fn le_bulletin_s_additionne_sur_mille_cas() {
        // Le bulletin doit toujours se lire : lignes = total, et
        // total + net = brut, au centime pres, jamais au-dela.
        let mut brut = d("0.01");
        let mut taux = d("0.5");
        for step in 0..1_000u32 {
            let b = bareme(vec![
                cotisation("A", Assiette::Brut, &taux.to_string()),
                cotisation("B", Assiette::BrutPlafonne, &taux.to_string()),
                cotisation("C", Assiette::Net, "1.5"),
            ]);
            let plafond_mensuel = if step % 2 == 0 { d("3000") } else { d("500") };

            let bulletin = calculer(&b, brut, plafond_mensuel).unwrap();

            let somme: Decimal = bulletin.lignes.iter().map(|l| l.montant).sum();
            assert_eq!(bulletin.total_cotisations, somme, "lignes = total a {brut}");
            assert_eq!(
                bulletin.total_cotisations + bulletin.net_a_payer,
                bulletin.brut,
                "total + net = brut a {brut}"
            );
            assert!(bulletin.net_a_payer >= Decimal::ZERO);
            for ligne in &bulletin.lignes {
                assert!(
                    ligne.montant.scale() <= SCALE_CENTIME,
                    "montant plus fin que le centime : {} ({})",
                    ligne.montant,
                    ligne.code
                );
                assert_eq!(
                    ligne.montant,
                    arrondi_centime(ligne.base * ligne.taux / d("100")),
                    "montant non rejouable a la main pour {}",
                    ligne.code
                );
            }

            brut += d("37.77");
            if taux < d("30") {
                taux += d("0.37");
            }
        }
    }
}
