//! Multi-devises : conversion exacte entre monnaies.
//!
//! C'est le moteur dont se sert tout document libelle dans une autre monnaie que
//! celle de l'entreprise : devis, facture, commande, paiement. Il repond a une
//! seule question — « 100 dollars, cela fait combien d'euros » — mais il la
//! repond de facon qu'un comptable puisse la signer.
//!
//! # Comment un taux est resolu
//!
//! Un [`Taux`] dit : 1 unite de `depuis` vaut `valeur` unites de `vers`. Pour une
//! conversion demandee, la table cherche dans cet ordre :
//!
//! 1. un **taux direct** `depuis -> vers` ;
//! 2. a defaut, l'**inverse** d'un taux `vers -> depuis` ;
//! 3. a defaut, le **chemin par la devise de base** : `depuis -> base -> vers`,
//!    en admettant l'inverse sur chacune des deux jambes.
//!
//! Aucune autre hypothese n'est faite : sans chemin, la conversion echoue. Elle
//! ne rend **jamais 0**, qui serait un montant plausible et faux — le pire
//! resultat possible sur une facture.
//!
//! # Ce que le calcul garantit
//!
//! - **Aucun flottant.** Tout passe par `rust_decimal` : 100 USD a 0,9234 font
//!   92,34 EUR, pas 92,33999999.
//! - **L'arrondi est commercial** ([`RoundingStrategy::MidpointAwayFromZero`]) :
//!   1,005 au centime donne 1,01 et jamais 1,00. L'arrondi bancaire
//!   (`MidpointNearestEven`) est celui de l'IEEE 754, pas celui d'une facture.
//! - **La conversion ne perd rien** : [`Table::convertir`] rend le produit exact,
//!   sans arrondi. C'est [`Table::arrondir`] — ou [`Table::convertir_arrondi`] —
//!   qui fixe le nombre de decimales de la devise cible, une seule fois, a la fin.
//! - **Les codes sont normalises** (majuscules, sans espace autour) : `usd`,
//!   `USD` et ` usd ` designent la meme devise.

use std::collections::BTreeMap;

use rust_decimal::{Decimal, RoundingStrategy};

/// Le plus grand nombre de decimales que `rust_decimal` sait porter.
/// Au-dela, `round_dp_with_strategy` panique : on borne plutot que de planter.
const MAX_DECIMALES: u32 = 28;

/// Normalise un code devise : sans espace autour, en majuscules.
fn cle(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

/// Ce qui a empeche la conversion ou l'arrondi.
#[derive(Debug, PartialEq, Eq)]
pub enum Erreur {
    /// Le code demande n'est pas dans la table.
    DeviseInconnue(String),
    /// Aucun chemin de conversion : ni direct, ni inverse, ni par la base.
    /// Les deux champs sont la devise de depart et celle d'arrivee demandees.
    TauxManquant(String, String),
    /// Taux nul ou negatif (refuse des la saisie), ou valeur si grande que le
    /// calcul depasserait la capacite de `rust_decimal`. Le detail dit lequel.
    TauxInvalide(String),
    /// Un taux valant zero aurait du etre inverse : on refuse au lieu de
    /// laisser `Decimal` paniquer sur une division par zero.
    DivisionParZero,
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::DeviseInconnue(code) => write!(f, "devise inconnue : {code}"),
            Erreur::TauxManquant(depuis, vers) => {
                write!(f, "aucun taux de {depuis} vers {vers}")
            }
            Erreur::TauxInvalide(detail) => write!(f, "taux invalide : {detail}"),
            Erreur::DivisionParZero => write!(f, "division par zero"),
        }
    }
}

impl std::error::Error for Erreur {}

/// Une monnaie : son code ISO, son nom lisible, et le nombre de decimales avec
/// lesquelles elle se presente (2 pour l'euro, 0 pour le yen, 3 pour le dinar).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Devise {
    pub code: String,
    pub nom: String,
    pub decimales: u32,
}

impl Devise {
    /// Une devise dont le code est normalise (majuscules, sans espace).
    pub fn nouvelle(code: &str, nom: &str, decimales: u32) -> Self {
        Self {
            code: cle(code),
            nom: nom.to_string(),
            decimales,
        }
    }
}

/// Un taux saisi : 1 unite de `depuis` vaut `valeur` unites de `vers`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taux {
    pub depuis: String,
    pub vers: String,
    pub valeur: Decimal,
}

impl Taux {
    /// Un taux dont les deux codes sont normalises (majuscules, sans espace).
    pub fn nouveau(depuis: &str, vers: &str, valeur: Decimal) -> Self {
        Self {
            depuis: cle(depuis),
            vers: cle(vers),
            valeur,
        }
    }
}

/// La table des devises et des taux connus.
#[derive(Clone, Debug)]
pub struct Table {
    base: Devise,
    devises: BTreeMap<String, Devise>,
    /// `taux[depuis][vers]` : uniquement les taux saisis, jamais les deduits.
    taux: BTreeMap<String, BTreeMap<String, Taux>>,
}

impl Table {
    /// Une table vide dont la devise de base est celle de l'entreprise. C'est
    /// elle qui sert de pivot quand deux devises n'ont pas de taux entre elles.
    pub fn nouvelle(base: Devise) -> Self {
        let mut devises = BTreeMap::new();
        let mut base = base;
        base.code = cle(&base.code);
        devises.insert(base.code.clone(), base.clone());
        Self {
            base,
            devises,
            taux: BTreeMap::new(),
        }
    }

    /// La devise de base (celle de la comptabilite).
    pub fn base(&self) -> &Devise {
        &self.base
    }

    /// Ajoute une devise, ou remplace la fiche existante si le code est deja
    /// connu (on corrige alors le nombre de decimales, par exemple).
    pub fn ajouter_devise(&mut self, devise: Devise) {
        let mut devise = devise;
        devise.code = cle(&devise.code);
        if devise.code == self.base.code {
            self.base = devise.clone();
        }
        self.devises.insert(devise.code.clone(), devise);
    }

    /// Enregistre un taux. Rend `false` et n'ecrit rien si le taux est nul ou
    /// negatif : un taux nul ferait une conversion a zero, donc un document faux.
    pub fn ajouter_taux(&mut self, taux: Taux) -> bool {
        self.enregistrer_taux(taux).is_ok()
    }

    /// Variante explicite de [`Table::ajouter_taux`], qui dit pourquoi le taux
    /// a ete refuse.
    pub fn enregistrer_taux(&mut self, taux: Taux) -> Result<(), Erreur> {
        if taux.valeur <= Decimal::ZERO {
            return Err(Erreur::TauxInvalide(taux.valeur.to_string()));
        }
        let depuis = cle(&taux.depuis);
        let vers = cle(&taux.vers);
        let taux = Taux {
            depuis: depuis.clone(),
            vers: vers.clone(),
            valeur: taux.valeur,
        };
        self.taux.entry(depuis).or_default().insert(vers, taux);
        Ok(())
    }

    /// La fiche d'une devise, ou [`Erreur::DeviseInconnue`].
    pub fn devise(&self, code: &str) -> Result<&Devise, Erreur> {
        self.devises
            .get(&cle(code))
            .ok_or_else(|| Erreur::DeviseInconnue(code.to_string()))
    }

    /// Le taux **saisi** entre deux devises, dans ce sens precis. Les taux
    /// deduits (inverse, chemin par la base) ne sont pas listes : ce sont des
    /// resultats de calcul, pas des donnees a afficher.
    pub fn taux_direct(&self, depuis: &str, vers: &str) -> Option<Decimal> {
        self.taux
            .get(&cle(depuis))
            .and_then(|vers_depuis| vers_depuis.get(&cle(vers)))
            .map(|taux| taux.valeur)
    }

    /// Toutes les devises, triees par code : de quoi remplir un ecran de saisie.
    pub fn lister_devises(&self) -> Vec<&Devise> {
        self.devises.values().collect()
    }

    /// Tous les taux saisis, tries par devise de depart puis d'arrivee.
    pub fn lister_taux(&self) -> Vec<&Taux> {
        self.taux
            .values()
            .flat_map(|par_vers| par_vers.values())
            .collect()
    }

    /// Convertit un montant sans arrondir : le produit exact est rendu tel quel,
    /// c'est a l'appelant de fixer les decimales avec [`Table::arrondir`].
    pub fn convertir(&self, montant: Decimal, depuis: &str, vers: &str) -> Result<Decimal, Erreur> {
        // Une devise inconnue doit etre dite avant toute histoire de taux :
        // « taux manquant » enverrait chercher une saisie qui n'a pas de sens.
        let depart = self.devise(depuis)?;
        let arrivee = self.devise(vers)?;

        let facteur = match self.chemin(&depart.code, &arrivee.code)? {
            Some(facteur) => facteur,
            None => return Err(Erreur::TauxManquant(depuis.to_string(), vers.to_string())),
        };

        multiplier(montant, facteur)
    }

    /// Convertit puis arrondit a la devise d'arrivee : le chemin normal d'un
    /// document, qui n'affiche pas plus de decimales que la monnaie n'en porte.
    pub fn convertir_arrondi(
        &self,
        montant: Decimal,
        depuis: &str,
        vers: &str,
    ) -> Result<Decimal, Erreur> {
        let converti = self.convertir(montant, depuis, vers)?;
        self.arrondir(converti, vers)
    }

    /// Arrondit au nombre de decimales de la devise, en **arrondi commercial**
    /// (0,5 s'eloigne de zero). Jamais l'arrondi bancaire : 1,005 -> 1,01.
    pub fn arrondir(&self, montant: Decimal, code: &str) -> Result<Decimal, Erreur> {
        let devise = self.devise(code)?;
        let decimales = devise.decimales.min(MAX_DECIMALES);
        Ok(montant.round_dp_with_strategy(decimales, RoundingStrategy::MidpointAwayFromZero))
    }

    /// Le facteur de conversion `depuis -> vers`, ou `None` s'il n'existe aucun
    /// chemin. Les cles recues sont deja normalisees.
    fn chemin(&self, depuis: &str, vers: &str) -> Result<Option<Decimal>, Erreur> {
        if depuis == vers {
            return Ok(Some(Decimal::ONE));
        }
        if let Some(facteur) = self.facteur_simple(depuis, vers)? {
            return Ok(Some(facteur));
        }

        // Chemin par la base. On interdit une troisieme jambe : `facteur_simple`
        // ne rappelle jamais `chemin`, donc pas de recursion ni de boucle.
        let base = self.base.code.as_str();
        if depuis == base || vers == base {
            return Ok(None);
        }
        let amont = match self.facteur_simple(depuis, base)? {
            Some(facteur) => facteur,
            None => return Ok(None),
        };
        let aval = match self.facteur_simple(base, vers)? {
            Some(facteur) => facteur,
            None => return Ok(None),
        };
        Ok(Some(multiplier(amont, aval)?))
    }

    /// Taux direct, sinon inverse. Une seule jambe, sans passer par la base.
    fn facteur_simple(&self, depuis: &str, vers: &str) -> Result<Option<Decimal>, Erreur> {
        if depuis == vers {
            return Ok(Some(Decimal::ONE));
        }
        if let Some(taux) = self
            .taux
            .get(depuis)
            .and_then(|par_vers| par_vers.get(vers))
        {
            return Ok(Some(taux.valeur));
        }
        if let Some(taux) = self
            .taux
            .get(vers)
            .and_then(|par_vers| par_vers.get(depuis))
        {
            return Ok(Some(inverser(taux.valeur)?));
        }
        Ok(None)
    }
}

/// Inverse un taux. Un taux nul est refuse explicitement : sans ce garde-fou,
/// `Decimal` panique (« Division by zero ») et emporte le serveur.
fn inverser(valeur: Decimal) -> Result<Decimal, Erreur> {
    if valeur.is_zero() {
        return Err(Erreur::DivisionParZero);
    }
    if valeur < Decimal::ZERO {
        return Err(Erreur::TauxInvalide(valeur.to_string()));
    }
    Decimal::ONE
        .checked_div(valeur)
        .ok_or_else(|| Erreur::TauxInvalide(valeur.to_string()))
}

/// Multiplication exacte qui ne panique pas : `Decimal` panique sur un
/// depassement de capacite, ce qui n'est pas une facon acceptable d'echouer sur
/// une facture.
fn multiplier(gauche: Decimal, droite: Decimal) -> Result<Decimal, Erreur> {
    gauche
        .checked_mul(droite)
        .ok_or_else(|| Erreur::TauxInvalide(droite.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal de test invalide")
    }

    /// Table de travail : base EUR, plus USD, CAD, JPY (0 decimale) et TND
    /// (3 decimales). Aucun taux n'y est saisi.
    fn table() -> Table {
        let mut t = Table::nouvelle(Devise::nouvelle("EUR", "Euro", 2));
        t.ajouter_devise(Devise::nouvelle("USD", "Dollar americain", 2));
        t.ajouter_devise(Devise::nouvelle("CAD", "Dollar canadien", 2));
        t.ajouter_devise(Devise::nouvelle("JPY", "Yen", 0));
        t.ajouter_devise(Devise::nouvelle("TND", "Dinar tunisien", 3));
        t
    }

    fn ecart(a: Decimal, b: Decimal) -> Decimal {
        (a - b).abs()
    }

    #[test]
    fn chemin_par_la_base_quand_aucun_taux_direct() {
        // Seuls USD -> EUR et EUR -> CAD existent : USD -> CAD doit passer par
        // l'euro et multiplier les deux taux.
        let mut t = table();
        assert!(t.ajouter_taux(Taux::nouveau("USD", "EUR", d("0.9234"))));
        assert!(t.ajouter_taux(Taux::nouveau("EUR", "CAD", d("1.5"))));

        // 100 USD = 92,34 EUR = 138,51 CAD
        assert_eq!(
            t.convertir(d("100"), "USD", "CAD")
                .expect("chemin par la base"),
            d("138.51")
        );
        assert_eq!(t.convertir(d("200"), "USD", "CAD").unwrap(), d("277.02"));

        // Le retour emprunte les deux inverses ; le centime reste juste.
        let retour = t.convertir(d("138.51"), "CAD", "USD").unwrap();
        assert_eq!(t.arrondir(retour, "USD").unwrap(), d("100"));
        assert!(ecart(retour, d("100")) < d("0.000000000001"));
    }

    #[test]
    fn taux_inverse_quand_seul_vers_depuis_existe() {
        // Seul EUR -> USD est saisi ; USD -> EUR doit en etre deduit.
        let mut t = table();
        assert!(t.ajouter_taux(Taux::nouveau("EUR", "USD", d("1.1"))));

        let inverse = t.convertir(d("110"), "USD", "EUR").expect("inverse deduit");
        assert_eq!(t.arrondir(inverse, "EUR").unwrap(), d("100"));
        assert!(ecart(inverse, d("100")) < d("0.000000000001"));

        // Le sens direct n'a pas bouge.
        assert_eq!(t.convertir(d("100"), "EUR", "USD").unwrap(), d("110"));
    }

    #[test]
    fn exactitude_decimale_sans_flottant() {
        let mut t = table();
        assert!(t.ajouter_taux(Taux::nouveau("USD", "EUR", d("0.9234"))));

        // 100 x 0,9234 = 92,34, et non 92,33999999 comme le donnerait un f64.
        let r = t.convertir(d("100"), "USD", "EUR").expect("100 USD");
        assert_eq!(r, d("92.34"));
        // Aucun residu binaire : le produit par 10 000 rend un entier exact.
        assert_eq!(r * d("10000"), d("923400"));

        // Meme piege que 0,1 + 0,2 : en flottant, 3 x 0,1 = 0,30000000000000004.
        let mut t2 = table();
        assert!(t2.ajouter_taux(Taux::nouveau("USD", "EUR", d("0.1"))));
        assert_eq!(t2.convertir(d("3"), "USD", "EUR").unwrap(), d("0.3"));
    }

    #[test]
    fn arrondi_commercial_et_non_bancaire() {
        let t = table();

        // LE piege : 1,005 au centime. Commercial -> 1,01 ; bancaire
        // (`MidpointNearestEven`) -> 1,00, parce que 0 est pair.
        assert_eq!(t.arrondir(d("1.005"), "EUR").unwrap(), d("1.01"));
        assert_eq!(t.arrondir(d("2.675"), "EUR").unwrap(), d("2.68"));
        assert_eq!(t.arrondir(d("1.004"), "EUR").unwrap(), d("1.00"));

        // Le demi s'eloigne de zero dans les deux sens.
        assert_eq!(t.arrondir(d("-1.005"), "EUR").unwrap(), d("-1.01"));

        // Devise sans decimale : le yen.
        assert_eq!(t.arrondir(d("1234.5"), "JPY").unwrap(), d("1235"));

        // Devise a trois decimales : le dinar tunisien.
        assert_eq!(t.arrondir(d("1.0005"), "TND").unwrap(), d("1.001"));
    }

    #[test]
    fn devise_inconnue_est_signalee() {
        let t = table();

        // Une devise inconnue jamais ne vaut zero : elle vaut une erreur, des
        // deux cotes de la conversion et a l'arrondi.
        assert_eq!(
            t.convertir(d("1"), "XXX", "EUR"),
            Err(Erreur::DeviseInconnue("XXX".to_string()))
        );
        assert_eq!(
            t.convertir(d("1"), "EUR", "XXX"),
            Err(Erreur::DeviseInconnue("XXX".to_string()))
        );
        assert_eq!(
            t.arrondir(d("1.005"), "XXX"),
            Err(Erreur::DeviseInconnue("XXX".to_string()))
        );
        assert!(matches!(
            t.convertir_arrondi(d("1"), "XXX", "EUR"),
            Err(Erreur::DeviseInconnue(_))
        ));
    }

    #[test]
    fn taux_manquant_est_signale() {
        // Aucun taux saisi : on ne peut pas convertir.
        let t = table();
        assert_eq!(
            t.convertir(d("100"), "USD", "CAD"),
            Err(Erreur::TauxManquant("USD".to_string(), "CAD".to_string()))
        );

        // Une seule jambe existe : le chemin par la base est incomplet, donc
        // c'est l'erreur, pas un demi-calcul.
        let mut t2 = table();
        assert!(t2.ajouter_taux(Taux::nouveau("USD", "EUR", d("0.9234"))));
        assert_eq!(
            t2.convertir(d("100"), "USD", "CAD"),
            Err(Erreur::TauxManquant("USD".to_string(), "CAD".to_string()))
        );
    }

    #[test]
    fn un_taux_nul_ou_negatif_est_refuse() {
        let mut t = table();

        assert!(!t.ajouter_taux(Taux::nouveau("USD", "EUR", Decimal::ZERO)));
        assert!(!t.ajouter_taux(Taux::nouveau("USD", "EUR", d("-0.5"))));

        // Rien n'a ete enregistre : le taux est toujours manquant.
        assert_eq!(
            t.convertir(d("1"), "USD", "EUR"),
            Err(Erreur::TauxManquant("USD".to_string(), "EUR".to_string()))
        );
        assert_eq!(t.taux_direct("USD", "EUR"), None);

        // La variante explicite dit pourquoi.
        assert_eq!(
            t.enregistrer_taux(Taux::nouveau("USD", "EUR", Decimal::ZERO)),
            Err(Erreur::TauxInvalide("0".to_string()))
        );
    }

    #[test]
    fn convertir_dans_la_meme_devise_ne_demande_aucun_taux() {
        // L'identite ne doit pas dependre d'une saisie : sans taux, 1 EUR = 1 EUR.
        let t = table();
        assert_eq!(
            t.convertir(d("1234.5678"), "EUR", "EUR").unwrap(),
            d("1234.5678")
        );
        assert_eq!(t.convertir(d("0.005"), "USD", "USD").unwrap(), d("0.005"));
    }

    #[test]
    fn le_taux_direct_prime_sur_le_chemin_par_la_base() {
        let mut t = table();
        assert!(t.ajouter_taux(Taux::nouveau("USD", "EUR", d("0.9"))));
        assert!(t.ajouter_taux(Taux::nouveau("EUR", "CAD", d("1.5"))));
        // Taux direct volontairement different de 0,9 x 1,5 = 1,35 : la saisie
        // explicite doit gagner, sinon on ne peut jamais corriger un cours.
        assert!(t.ajouter_taux(Taux::nouveau("USD", "CAD", d("1.4"))));
        assert_eq!(t.convertir(d("100"), "USD", "CAD").unwrap(), d("140"));
    }

    #[test]
    fn les_codes_sont_normalises() {
        let mut t = Table::nouvelle(Devise::nouvelle(" eur ", "Euro", 2));
        t.ajouter_devise(Devise::nouvelle("usd", "Dollar americain", 2));
        assert!(t.ajouter_taux(Taux::nouveau("usd", "EUR", d("0.9234"))));

        assert_eq!(t.base().code, "EUR");
        assert_eq!(t.devise("usd").unwrap().code, "USD");
        assert_eq!(t.convertir(d("100"), "USD", "eur").unwrap(), d("92.34"));
    }

    #[test]
    fn un_montant_hors_capacite_remonte_une_erreur_au_lieu_de_paniquer() {
        let mut t = table();
        assert!(t.ajouter_taux(Taux::nouveau("USD", "EUR", d("2"))));

        // Decimal::MAX x 2 sort de la capacite de rust_decimal.
        assert_eq!(
            t.convertir(Decimal::MAX, "USD", "EUR"),
            Err(Erreur::TauxInvalide(d("2").to_string()))
        );
    }

    #[test]
    fn conversion_arrondie_a_la_devise_cible() {
        let mut t = table();
        assert!(t.ajouter_taux(Taux::nouveau("USD", "EUR", d("0.9234"))));
        assert!(t.ajouter_taux(Taux::nouveau("EUR", "JPY", d("163.42"))));

        assert_eq!(
            t.convertir_arrondi(d("100"), "USD", "EUR").unwrap(),
            d("92.34")
        );
        // 100 USD = 92,34 EUR = 15 090,2028 JPY -> 15 090 yens (devise a 0 decimale).
        assert_eq!(
            t.convertir_arrondi(d("100"), "USD", "JPY").unwrap(),
            d("15090")
        );
    }

    #[test]
    fn les_tables_se_listent_pour_un_ecran_de_saisie() {
        let mut t = table();
        assert!(t.ajouter_taux(Taux::nouveau("USD", "EUR", d("0.9234"))));

        assert_eq!(t.lister_devises().len(), 5);
        assert_eq!(t.lister_devises()[0].code, "CAD");

        let taux = t.lister_taux();
        assert_eq!(taux.len(), 1);
        assert_eq!(taux[0].depuis, "USD");
        assert_eq!(taux[0].vers, "EUR");
        assert_eq!(taux[0].valeur, d("0.9234"));

        // Un taux deduit n'apparait pas dans la liste : ce n'est pas une donnee.
        assert_eq!(t.taux_direct("EUR", "USD"), None);
    }

    #[test]
    fn les_erreurs_s_expliquent_en_francais() {
        assert_eq!(
            Erreur::DeviseInconnue("XXX".to_string()).to_string(),
            "devise inconnue : XXX"
        );
        assert_eq!(
            Erreur::TauxManquant("USD".to_string(), "CAD".to_string()).to_string(),
            "aucun taux de USD vers CAD"
        );
        assert_eq!(
            Erreur::TauxInvalide("0".to_string()).to_string(),
            "taux invalide : 0"
        );
        assert_eq!(Erreur::DivisionParZero.to_string(), "division par zero");

        // Implemente std::error::Error : utilisable avec `?` et `Box<dyn Error>`.
        let boite: Box<dyn std::error::Error> = Box::new(Erreur::DivisionParZero);
        assert_eq!(boite.to_string(), "division par zero");
    }
}
