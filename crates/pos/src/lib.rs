//! Moteur de caisse (point de vente) sans ecran : session de vente, panier,
//! encaissement, cloture et ecart de caisse.
//!
//! Regles reproduites, chacune couverte par un test :
//! - la TVA est arrondie **par ligne** a 2 decimales, jamais sur le total. On
//!   utilise le demi hors de zero (`MidpointAwayFromZero`), equivalent de
//!   `round()` de PHP que Dolibarr applique dans `price2num()`. Trois lignes a
//!   19,99 EUR a 20 % donnent 12,00 EUR de TVA ligne a ligne contre 11,99 EUR
//!   si on arrondit la TVA du total : 1 centime d'ecart, c'est la regle
//!   Dolibarr et c'est ce qui est teste ;
//! - la remise s'applique au HT, ligne par ligne ; la TVA est ensuite
//!   recalculee sur le HT remise, donc le TTC suit la remise au centime
//!   (7,5 % de remise donnent 7,5 % de moins sur chaque total ligne) ;
//! - le panier reste coherent : `total_ttc == total_ht + total_tva` a chaque
//!   instant, puisque chaque ligne est arrondie separement puis additionnee ;
//! - le rendu ne peut etre negatif : un encaissement incomplet donne un rendu
//!   de 0 et un solde du ;
//! - l'ecart de caisse vaut `comptage - (fond de caisse + especes encaissees -
//!   rendu)` : positif pour un excedent, negatif pour un manquant.
//!
//! Le rendu est toujours suppose sortir de la caisse en especes, quelle que
//! soit la ventilation des paiements (c'est la pratique au comptoir).

use rust_decimal::{Decimal, RoundingStrategy};

/// Nombre de decimales d'un montant en euros.
pub const DECIMALES_MONTANT: u32 = 2;

/// Nombre de decimales d'un pourcentage de remise.
pub const DECIMALES_POURCENTAGE: u32 = 4;

/// Arrondi monetaire a 2 decimales, demi hors de zero (`round()` de PHP).
pub fn arrondi_centime(valeur: Decimal) -> Decimal {
    valeur.round_dp_with_strategy(DECIMALES_MONTANT, RoundingStrategy::MidpointAwayFromZero)
}

/// Arrondi d'un pourcentage, demi hors de zero.
pub fn arrondi_pourcentage(valeur: Decimal) -> Decimal {
    valeur.round_dp_with_strategy(
        DECIMALES_POURCENTAGE,
        RoundingStrategy::MidpointAwayFromZero,
    )
}

/// Moyen de paiement en especes, tel qu'il est enregistre par la caisse.
pub const MOYEN_ESPECES: &str = "especes";

/// Normalise un libelle de moyen de paiement : minuscules, sans espaces
/// autour, accents ramenes a la lettre de base. `Especes`, `ESPECES` et la
/// variante accentuee en "e" grave designent donc le meme moyen.
pub fn normalise_moyen(moyen: &str) -> String {
    moyen
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            autre => autre,
        })
        .collect()
}

/// Vrai si le moyen de paiement alimente le tiroir-caisse.
pub fn est_especes(moyen: &str) -> bool {
    let n = normalise_moyen(moyen);
    n == MOYEN_ESPECES || n == "espece" || n == "cash" || n == "liquide"
}

/// Erreurs de la caisse. Chaque variante porte les valeurs fautives, pour que
/// l'ecran ou le journal puisse les afficher sans reconstruire le contexte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erreur {
    /// Quantite nulle ou negative sur une ligne.
    QuantiteInvalide {
        reference: String,
        quantite: Decimal,
    },
    /// Prix unitaire ou taux de TVA negatif sur une ligne.
    PrixInvalide { reference: String, prix: Decimal },
    /// Aucune ligne ne porte cette reference.
    LigneInconnue { reference: String },
    /// Remise hors de l'intervalle `[0 ; 100]`.
    RemiseInvalide { pourcentage: Decimal },
    /// Montant de paiement nul ou negatif.
    PaiementInvalide { montant: Decimal },
    /// Moyen de paiement vide.
    MoyenInvalide,
    /// Fond de caisse ou comptage negatif.
    MontantInvalide {
        champ: &'static str,
        valeur: Decimal,
    },
    /// Cloture refusee tant que la vente n'est pas entierement payee.
    VenteNonSoldee { solde_du: Decimal },
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::QuantiteInvalide {
                reference,
                quantite,
            } => write!(
                f,
                "quantite invalide pour la ligne {reference} : {quantite} (doit etre > 0)"
            ),
            Erreur::PrixInvalide { reference, prix } => write!(
                f,
                "prix ou taux de TVA negatif pour la ligne {reference} : {prix}"
            ),
            Erreur::LigneInconnue { reference } => {
                write!(f, "aucune ligne {reference} dans le panier")
            }
            Erreur::RemiseInvalide { pourcentage } => write!(
                f,
                "remise invalide : {pourcentage} % (doit etre entre 0 et 100)"
            ),
            Erreur::PaiementInvalide { montant } => {
                write!(
                    f,
                    "montant de paiement invalide : {montant} (doit etre > 0)"
                )
            }
            Erreur::MoyenInvalide => write!(f, "moyen de paiement vide"),
            Erreur::MontantInvalide { champ, valeur } => {
                write!(f, "{champ} invalide : {valeur} (doit etre >= 0)")
            }
            Erreur::VenteNonSoldee { solde_du } => {
                write!(f, "vente non soldee : il reste {solde_du} EUR a encaisser")
            }
        }
    }
}

impl std::error::Error for Erreur {}

/// Une ligne de ticket : article, quantite, prix unitaire HT, taux de TVA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ligne {
    pub reference: String,
    pub libelle: String,
    pub quantite: Decimal,
    pub prix_unitaire_ht: Decimal,
    pub taux_tva: Decimal,
}

impl Ligne {
    /// Construit une ligne et valide quantite (> 0), prix unitaire et taux
    /// (>= 0). C'est le seul constructeur : une `Ligne` ne peut pas exister
    /// avec une quantite nulle.
    pub fn new(
        reference: impl Into<String>,
        libelle: impl Into<String>,
        quantite: Decimal,
        prix_unitaire_ht: Decimal,
        taux_tva: Decimal,
    ) -> Result<Self, Erreur> {
        let ligne = Self {
            reference: reference.into(),
            libelle: libelle.into(),
            quantite,
            prix_unitaire_ht,
            taux_tva,
        };
        ligne.valider()?;
        Ok(ligne)
    }

    /// Verifie les invariants de la ligne.
    pub fn valider(&self) -> Result<(), Erreur> {
        if self.quantite <= Decimal::ZERO {
            return Err(Erreur::QuantiteInvalide {
                reference: self.reference.clone(),
                quantite: self.quantite,
            });
        }
        if self.prix_unitaire_ht < Decimal::ZERO {
            return Err(Erreur::PrixInvalide {
                reference: self.reference.clone(),
                prix: self.prix_unitaire_ht,
            });
        }
        if self.taux_tva < Decimal::ZERO {
            return Err(Erreur::PrixInvalide {
                reference: self.reference.clone(),
                prix: self.taux_tva,
            });
        }
        Ok(())
    }
}

/// Montants d'une ligne apres remise et arrondis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TotauxLigne {
    /// `quantite * prix_unitaire_ht`, arrondi au centime (avant remise).
    pub ht_brut: Decimal,
    /// HT apres remise.
    pub ht: Decimal,
    /// TVA calculee sur le HT remise, arrondie au centime.
    pub tva: Decimal,
    /// `ht + tva`, donc exactement coherent au centime.
    pub ttc: Decimal,
}

/// Panier de la session : lignes + remise globale en pourcentage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Panier {
    lignes: Vec<Ligne>,
    remise_pct: Decimal,
}

impl Panier {
    pub fn new() -> Self {
        Self {
            lignes: Vec::new(),
            remise_pct: Decimal::ZERO,
        }
    }

    /// Ajoute une ligne et renvoie `true` si la reference etait deja presente
    /// (auquel cas la ligne est empilee, pas fusionnee : deux prix differents
    /// pour la meme reference doivent rester visibles sur le ticket).
    pub fn ajouter(&mut self, ligne: Ligne) -> Result<bool, Erreur> {
        ligne.valider()?;
        let doublon = self.lignes.iter().any(|l| l.reference == ligne.reference);
        self.lignes.push(ligne);
        Ok(doublon)
    }

    /// Retire la premiere ligne portant cette reference.
    pub fn retirer(&mut self, reference: &str) -> Result<Ligne, Erreur> {
        let position = self
            .lignes
            .iter()
            .position(|l| l.reference == reference)
            .ok_or_else(|| Erreur::LigneInconnue {
                reference: reference.to_string(),
            })?;
        Ok(self.lignes.remove(position))
    }

    /// Vide les lignes et remet la remise a zero.
    pub fn vider(&mut self) {
        self.lignes.clear();
        self.remise_pct = Decimal::ZERO;
    }

    pub fn lignes(&self) -> &[Ligne] {
        &self.lignes
    }

    pub fn est_vide(&self) -> bool {
        self.lignes.is_empty()
    }

    pub fn nombre_lignes(&self) -> usize {
        self.lignes.len()
    }

    /// Remise globale en cours, en pourcentage.
    pub fn remise_pct_actuelle(&self) -> Decimal {
        self.remise_pct
    }

    /// Applique une remise globale en pourcentage sur le HT. Refusee hors de
    /// `[0 ; 100]` ; le panier reste inchange en cas de refus.
    pub fn remise_pct(&mut self, pourcentage: Decimal) -> Result<(), Erreur> {
        if pourcentage < Decimal::ZERO || pourcentage > Decimal::from(100) {
            return Err(Erreur::RemiseInvalide { pourcentage });
        }
        self.remise_pct = arrondi_pourcentage(pourcentage);
        Ok(())
    }

    /// Totaux d'une ligne : quantite x prix, remise, TVA par ligne.
    pub fn totaux_ligne(&self, ligne: &Ligne) -> TotauxLigne {
        let cent = Decimal::from(100);
        let ht_brut = arrondi_centime(ligne.quantite * ligne.prix_unitaire_ht);
        // Remise appliquee au HT, puis TVA recalculee sur le HT remise : le TTC
        // suit donc la remise au centime.
        let ht = arrondi_centime(ht_brut * (cent - self.remise_pct) / cent);
        let tva = arrondi_centime(ht * ligne.taux_tva / cent);
        TotauxLigne {
            ht_brut,
            ht,
            tva,
            ttc: ht + tva,
        }
    }

    /// Detail ligne a ligne (pour le ticket et pour le controle de TVA).
    pub fn detail(&self) -> Vec<TotauxLigne> {
        self.lignes
            .iter()
            .map(|ligne| self.totaux_ligne(ligne))
            .collect()
    }

    /// Somme des HT de lignes (remise appliquee).
    pub fn total_ht(&self) -> Decimal {
        self.detail().iter().map(|t| t.ht).sum()
    }

    /// Somme des TVA arrondies ligne a ligne : jamais la TVA du total.
    pub fn total_tva(&self) -> Decimal {
        self.detail().iter().map(|t| t.tva).sum()
    }

    /// Somme des TTC de lignes, egale a `total_ht + total_tva` au centime.
    pub fn total_ttc(&self) -> Decimal {
        self.detail().iter().map(|t| t.ttc).sum()
    }

    /// TVA telle qu'on l'obtiendrait en arrondissant le total global : sert au
    /// controle, pas au calcul (c'est la source de l'ecart d'1 centime).
    pub fn tva_sur_total_global(&self) -> Decimal {
        let cent = Decimal::from(100);
        let detail = self.detail();
        let tva_brute: Decimal = self
            .lignes
            .iter()
            .zip(detail.iter())
            .map(|(ligne, totaux)| totaux.ht * ligne.taux_tva / cent)
            .sum();
        arrondi_centime(tva_brute)
    }
}

/// Encaissement : moyen de paiement libre ("especes", "cb", "cheque"...) et
/// montant positif.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paiement {
    pub moyen: String,
    pub montant: Decimal,
}

impl Paiement {
    /// Construit un paiement en refusant un moyen vide et un montant <= 0.
    pub fn new(moyen: impl Into<String>, montant: Decimal) -> Result<Self, Erreur> {
        let moyen = moyen.into();
        if normalise_moyen(&moyen).is_empty() {
            return Err(Erreur::MoyenInvalide);
        }
        if montant <= Decimal::ZERO {
            return Err(Erreur::PaiementInvalide { montant });
        }
        Ok(Self {
            moyen,
            montant: arrondi_centime(montant),
        })
    }

    pub fn est_especes(&self) -> bool {
        est_especes(&self.moyen)
    }
}

/// Cloture de caisse : ce qui est vendu, ce qui devrait etre dans le tiroir,
/// ce qui y est reellement, et l'ecart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cloture {
    /// Total TTC des ventes de la session.
    pub total_ventes_ttc: Decimal,
    /// Fond de caisse + especes encaissees - rendu.
    pub total_especes_theorique: Decimal,
    /// Especes comptees par le caissier.
    pub comptage: Decimal,
    /// `comptage - total_especes_theorique` : positif = excedent, negatif =
    /// manquant.
    pub ecart: Decimal,
}

/// Session de vente : un panier et ses encaissements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    panier: Panier,
    paiements: Vec<Paiement>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            panier: Panier::new(),
            paiements: Vec::new(),
        }
    }

    pub fn panier(&self) -> &Panier {
        &self.panier
    }

    pub fn panier_mut(&mut self) -> &mut Panier {
        &mut self.panier
    }

    pub fn paiements(&self) -> &[Paiement] {
        &self.paiements
    }

    /// Total TTC a payer pour l'instant.
    pub fn total_ttc(&self) -> Decimal {
        self.panier.total_ttc()
    }

    /// Somme des paiements enregistres.
    pub fn total_paye(&self) -> Decimal {
        self.paiements.iter().map(|p| p.montant).sum()
    }

    /// Somme des paiements en especes (alimente le tiroir-caisse).
    pub fn total_especes_encaissees(&self) -> Decimal {
        self.paiements
            .iter()
            .filter(|p| p.est_especes())
            .map(|p| p.montant)
            .sum()
    }

    /// Encaisse un montant. Refuse un moyen vide et un montant <= 0 ; accepte
    /// volontairement le surpaiement, c'est `rendu()` qui le traite.
    pub fn encaisser(&mut self, moyen: &str, montant: Decimal) -> Result<(), Erreur> {
        self.paiements.push(Paiement::new(moyen, montant)?);
        Ok(())
    }

    /// Monnaie a rendre : `paiements - TTC`, jamais negatif (0 si la vente
    /// n'est pas encore couverte).
    pub fn rendu(&self) -> Decimal {
        let excedent = self.total_paye() - self.total_ttc();
        if excedent > Decimal::ZERO {
            arrondi_centime(excedent)
        } else {
            Decimal::ZERO
        }
    }

    /// Reste a payer : `TTC - paiements`, jamais negatif.
    pub fn solde_du(&self) -> Decimal {
        let reste = self.total_ttc() - self.total_paye();
        if reste > Decimal::ZERO {
            arrondi_centime(reste)
        } else {
            Decimal::ZERO
        }
    }

    /// Cloture la session et calcule l'ecart de caisse.
    ///
    /// `ecart = comptage - (fond_initial + especes encaissees - rendu)`.
    /// Refusee si la vente n'est pas soldee (sinon on cloturerait une vente
    /// dont l'argent n'est pas entre) ou si un montant est negatif.
    pub fn cloturer(
        &mut self,
        fond_initial: Decimal,
        comptage_especes: Decimal,
    ) -> Result<Cloture, Erreur> {
        if fond_initial < Decimal::ZERO {
            return Err(Erreur::MontantInvalide {
                champ: "fond de caisse",
                valeur: fond_initial,
            });
        }
        if comptage_especes < Decimal::ZERO {
            return Err(Erreur::MontantInvalide {
                champ: "comptage des especes",
                valeur: comptage_especes,
            });
        }
        let solde_du = self.solde_du();
        if solde_du > Decimal::ZERO {
            return Err(Erreur::VenteNonSoldee { solde_du });
        }

        let total_ventes_ttc = self.total_ttc();
        let total_especes_theorique =
            arrondi_centime(fond_initial + self.total_especes_encaissees() - self.rendu());
        let comptage = arrondi_centime(comptage_especes);

        Ok(Cloture {
            total_ventes_ttc,
            total_especes_theorique,
            comptage,
            ecart: comptage - total_especes_theorique,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Lecture d'un decimal litteral dans les tests uniquement.
    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal de test invalide")
    }

    fn ligne(reference: &str, quantite: &str, prix: &str, taux: &str) -> Ligne {
        Ligne::new(reference, "Article", d(quantite), d(prix), d(taux))
            .expect("ligne de test valide")
    }

    // ---------------------------------------------------------------- regle 1
    // TVA arrondie par ligne, pas sur le total.

    #[test]
    fn tva_arrondie_par_ligne_et_non_sur_le_total() {
        let mut panier = Panier::new();
        for i in 0..3 {
            panier
                .ajouter(
                    Ligne::new(
                        format!("ART{i}"),
                        "Article a 19,99",
                        Decimal::ONE,
                        d("19.99"),
                        d("20"),
                    )
                    .expect("ligne valide"),
                )
                .expect("ajout");
        }

        // Par ligne : 19,99 x 20 % = 3,998 -> 4,00, donc 3 x 4,00 = 12,00.
        let detail = panier.detail();
        assert_eq!(detail.len(), 3);
        for totaux in &detail {
            assert_eq!(totaux.ht, d("19.99"));
            assert_eq!(totaux.tva, d("4.00"));
            assert_eq!(totaux.ttc, d("23.99"));
        }
        assert_eq!(panier.total_ht(), d("59.97"));
        assert_eq!(panier.total_tva(), d("12.00"));
        assert_eq!(panier.total_ttc(), d("71.97"));
        assert_eq!(panier.total_ht() + panier.total_tva(), panier.total_ttc());

        // Sur le total global : 59,97 x 20 % = 11,994 -> 11,99, soit 1 centime
        // de moins. L'ecart est donc de 0,01 EUR et c'est exactement pourquoi
        // on arrondit par ligne.
        assert_eq!(panier.tva_sur_total_global(), d("11.99"));
        let ecart = panier.total_tva() - panier.tva_sur_total_global();
        assert_eq!(ecart, d("0.01"));
        assert_ne!(panier.total_tva(), panier.tva_sur_total_global());
    }

    #[test]
    fn tva_par_ligne_sur_taux_multiples() {
        // 10 % et 5,5 % melanges : chaque taux est arrondi sur sa ligne, la
        // somme des lignes n'est pas arrondie une seconde fois.
        let mut panier = Panier::new();
        panier
            .ajouter(ligne("A", "1", "19.99", "10"))
            .expect("ajout");
        panier
            .ajouter(ligne("B", "1", "19.99", "20"))
            .expect("ajout");
        panier
            .ajouter(ligne("C", "1", "19.99", "5.5"))
            .expect("ajout");

        // 19,99 x 10 % = 1,999 -> 2,00 ; x 20 % = 3,998 -> 4,00 ;
        // x 5,5 % = 1,09945 -> 1,10.
        assert_eq!(panier.total_tva(), d("7.10"));
        assert_eq!(panier.total_ht(), d("59.97"));
        assert_eq!(panier.total_ttc(), d("67.07"));
        assert_eq!(panier.total_ht() + panier.total_tva(), panier.total_ttc());
    }

    // ---------------------------------------------------------------- regle 2
    // Remise appliquee au HT, la TVA suit.

    #[test]
    fn remise_de_7_5_pour_cent_suit_la_tva_et_le_ttc() {
        let mut panier = Panier::new();
        panier
            .ajouter(ligne("A", "4", "25.00", "20"))
            .expect("ajout");
        // Sans remise : 100,00 HT / 20,00 TVA / 120,00 TTC.
        assert_eq!(panier.total_ht(), d("100.00"));
        assert_eq!(panier.total_tva(), d("20.00"));
        assert_eq!(panier.total_ttc(), d("120.00"));

        panier.remise_pct(d("7.5")).expect("remise valide");
        // 100,00 x 0,925 = 92,50 HT ; 92,50 x 20 % = 18,50 TVA ; 111,00 TTC.
        assert_eq!(panier.remise_pct_actuelle(), d("7.5"));
        assert_eq!(panier.total_ht(), d("92.50"));
        assert_eq!(panier.total_tva(), d("18.50"));
        assert_eq!(panier.total_ttc(), d("111.00"));

        // Le TTC divise par 7,5 % : 120,00 x 0,925 = 111,00, au centime.
        assert_eq!(arrondi_centime(d("120.00") * d("0.925")), d("111.00"));
        assert_eq!(panier.total_ht() + panier.total_tva(), panier.total_ttc());
    }

    #[test]
    fn remise_de_10_pour_cent_divise_le_ttc_par_10_pour_cent() {
        let mut panier = Panier::new();
        panier
            .ajouter(ligne("A", "3", "33.33", "20"))
            .expect("ajout");
        let ttc_plein = panier.total_ttc();
        panier.remise_pct(d("10")).expect("remise valide");
        let ttc_remise = panier.total_ttc();
        // 99,99 TTC -> remise 10 % -> 89,99 TTC au centime.
        assert_eq!(ttc_plein, d("119.99"));
        assert_eq!(ttc_remise, d("107.99"));
        assert_eq!(
            arrondi_centime(ttc_plein * d("0.9")),
            ttc_remise,
            "une remise de 10 % doit retirer 10 % du TTC"
        );
    }

    // ---------------------------------------------------------------- regle 3
    // Surpaiement et rendu.

    #[test]
    fn surpaiement_en_especes_donne_un_rendu_exact() {
        let mut session = Session::new();
        session
            .panier_mut()
            .ajouter(ligne("MENU", "1", "35.25", "20"))
            .expect("ajout");
        // 35,25 HT x 1,20 = 42,30 TTC.
        assert_eq!(session.total_ttc(), d("42.30"));

        session.encaisser("especes", d("50.00")).expect("paiement");
        assert_eq!(session.rendu(), d("7.70"));
        assert_eq!(session.solde_du(), Decimal::ZERO);
        assert_eq!(session.total_especes_encaissees(), d("50.00"));
    }

    #[test]
    fn paiement_incomplet_donne_rendu_zero_et_solde_du() {
        let mut session = Session::new();
        session
            .panier_mut()
            .ajouter(ligne("MENU", "1", "35.25", "20"))
            .expect("ajout");
        session.encaisser("especes", d("20.00")).expect("paiement");

        assert_eq!(
            session.rendu(),
            Decimal::ZERO,
            "le rendu n'est jamais negatif"
        );
        assert_eq!(session.solde_du(), d("22.30"));
    }

    #[test]
    fn paiement_mixte_sans_surpaiement() {
        let mut session = Session::new();
        session
            .panier_mut()
            .ajouter(ligne("MENU", "1", "35.25", "20"))
            .expect("ajout");
        session.encaisser("cb", d("40.00")).expect("paiement");
        session.encaisser("especes", d("2.30")).expect("paiement");

        assert_eq!(session.solde_du(), Decimal::ZERO);
        assert_eq!(session.rendu(), Decimal::ZERO);
        assert_eq!(session.total_especes_encaissees(), d("2.30"));
    }

    // ---------------------------------------------------------------- regle 4
    // Cloture et ecart de caisse.

    #[test]
    fn cloture_avec_excedent_de_2_50() {
        let mut session = Session::new();
        session
            .panier_mut()
            .ajouter(ligne("MENU", "1", "35.25", "20"))
            .expect("ajout");
        session.encaisser("especes", d("50.00")).expect("paiement");

        // Theorique : 100,00 de fond + 50,00 encaisses - 7,70 rendus = 142,30.
        let cloture = session
            .cloturer(d("100.00"), d("144.80"))
            .expect("cloture valide");
        assert_eq!(cloture.total_ventes_ttc, d("42.30"));
        assert_eq!(cloture.total_especes_theorique, d("142.30"));
        assert_eq!(cloture.comptage, d("144.80"));
        assert_eq!(cloture.ecart, d("2.50"), "excedent de caisse");
    }

    #[test]
    fn cloture_avec_manquant_de_1_00() {
        let mut session = Session::new();
        session
            .panier_mut()
            .ajouter(ligne("MENU", "1", "35.25", "20"))
            .expect("ajout");
        session.encaisser("especes", d("50.00")).expect("paiement");

        let cloture = session
            .cloturer(d("100.00"), d("141.30"))
            .expect("cloture valide");
        assert_eq!(cloture.total_especes_theorique, d("142.30"));
        assert_eq!(cloture.comptage, d("141.30"));
        assert_eq!(cloture.ecart, d("-1.00"), "manquant de caisse");
    }

    #[test]
    fn cloture_refusee_si_la_vente_n_est_pas_soldee() {
        let mut session = Session::new();
        session
            .panier_mut()
            .ajouter(ligne("MENU", "1", "35.25", "20"))
            .expect("ajout");
        session.encaisser("especes", d("20.00")).expect("paiement");

        let erreur = session
            .cloturer(d("100.00"), d("120.00"))
            .expect_err("cloture refusee");
        assert_eq!(
            erreur,
            Erreur::VenteNonSoldee {
                solde_du: d("22.30")
            }
        );
    }

    // ---------------------------------------------------------------- regle 5
    // Refus : quantite <= 0, paiement <= 0, remise > 100 %.

    #[test]
    fn refus_quantite_nulle_ou_negative() {
        for quantite in ["0", "-1", "-0.5"] {
            let erreur = Ligne::new("A", "Article", d(quantite), d("10.00"), d("20"))
                .expect_err("quantite refusee");
            assert!(
                matches!(erreur, Erreur::QuantiteInvalide { .. }),
                "quantite {quantite} doit etre refusee"
            );
            assert!(erreur.to_string().contains("quantite invalide"));
        }

        // Le panier revalide la ligne meme si elle a ete forgee champ a champ.
        let mut panier = Panier::new();
        let mut forgee = ligne("A", "1", "10.00", "20");
        forgee.quantite = Decimal::ZERO;
        let erreur = panier.ajouter(forgee).expect_err("ajout refuse");
        assert!(matches!(erreur, Erreur::QuantiteInvalide { .. }));
        assert!(panier.est_vide());
    }

    #[test]
    fn refus_montant_de_paiement_nul_ou_negatif() {
        let mut session = Session::new();
        session
            .panier_mut()
            .ajouter(ligne("MENU", "1", "35.25", "20"))
            .expect("ajout");

        for montant in ["0", "-5.00"] {
            let erreur = session
                .encaisser("especes", d(montant))
                .expect_err("paiement refuse");
            assert!(
                matches!(erreur, Erreur::PaiementInvalide { .. }),
                "montant {montant} doit etre refuse"
            );
            assert!(erreur.to_string().contains("paiement invalide"));
        }
        assert!(session.paiements().is_empty());

        // Moyen vide refuse aussi : un paiement sans moyen n'est pas tracable.
        let erreur = session
            .encaisser("   ", d("10.00"))
            .expect_err("moyen refuse");
        assert_eq!(erreur, Erreur::MoyenInvalide);
        assert!(session.paiements().is_empty());
    }

    #[test]
    fn refus_remise_superieure_a_100_pour_cent() {
        let mut panier = Panier::new();
        panier
            .ajouter(ligne("A", "1", "10.00", "20"))
            .expect("ajout");

        for pourcentage in ["100.01", "150", "-1"] {
            let erreur = panier
                .remise_pct(d(pourcentage))
                .expect_err("remise refusee");
            assert_eq!(
                erreur,
                Erreur::RemiseInvalide {
                    pourcentage: d(pourcentage)
                }
            );
            // Le panier reste inchange apres refus.
            assert_eq!(panier.remise_pct_actuelle(), Decimal::ZERO);
            assert_eq!(panier.total_ttc(), d("12.00"));
        }

        // Bornes acceptees : 0 % et 100 %.
        panier.remise_pct(d("0")).expect("0 % valide");
        assert_eq!(panier.total_ttc(), d("12.00"));
        panier.remise_pct(d("100")).expect("100 % valide");
        assert_eq!(panier.total_ttc(), Decimal::ZERO);
    }

    // ------------------------------------------------------------- vie du panier

    #[test]
    fn retirer_et_vider_le_panier() {
        let mut panier = Panier::new();
        panier
            .ajouter(ligne("A", "1", "10.00", "20"))
            .expect("ajout");
        panier
            .ajouter(ligne("B", "2", "5.00", "20"))
            .expect("ajout");
        panier.remise_pct(d("10")).expect("remise");
        assert_eq!(panier.nombre_lignes(), 2);
        assert_eq!(panier.total_ttc(), d("21.60"));

        let retiree = panier.retirer("A").expect("retrait");
        assert_eq!(retiree.reference, "A");
        assert_eq!(panier.nombre_lignes(), 1);
        // La remise survit au retrait d'une ligne.
        assert_eq!(panier.remise_pct_actuelle(), d("10"));
        assert_eq!(panier.total_ttc(), d("10.80"));

        let erreur = panier.retirer("A").expect_err("ligne absente");
        assert_eq!(
            erreur,
            Erreur::LigneInconnue {
                reference: "A".to_string()
            }
        );

        panier.vider();
        assert!(panier.est_vide());
        assert_eq!(panier.remise_pct_actuelle(), Decimal::ZERO);
        assert_eq!(panier.total_ttc(), Decimal::ZERO);
    }

    #[test]
    fn doublon_de_reference_est_signale_mais_accepte() {
        let mut panier = Panier::new();
        assert!(!panier
            .ajouter(ligne("A", "1", "10.00", "20"))
            .expect("ajout"));
        assert!(panier
            .ajouter(ligne("A", "2", "9.00", "20"))
            .expect("ajout"));
        assert_eq!(panier.nombre_lignes(), 2);
        assert_eq!(panier.total_ht(), d("28.00"));
    }

    #[test]
    fn quantite_decimale_et_tva_arrondie_une_fois_par_ligne() {
        // 1,5 x 19,99 = 29,985 -> 29,99 HT ; 29,99 x 10 % = 2,999 -> 3,00 ;
        // TTC = 32,99. La ligne reste coherente au centime.
        let mut panier = Panier::new();
        panier
            .ajouter(ligne("A", "1.5", "19.99", "10"))
            .expect("ajout");
        assert_eq!(panier.total_ht(), d("29.99"));
        assert_eq!(panier.total_tva(), d("3.00"));
        assert_eq!(panier.total_ttc(), d("32.99"));
        assert_eq!(panier.total_ht() + panier.total_tva(), panier.total_ttc());
    }

    #[test]
    fn total_ttc_egal_ht_plus_tva_sur_un_millier_de_paniers() {
        let mut quantite = d("0.37");
        let mut prix = d("1.13");
        let mut remise = d("0");
        for _ in 0..1_000u32 {
            let mut panier = Panier::new();
            panier
                .ajouter(ligne("A", &quantite.to_string(), &prix.to_string(), "20"))
                .expect("ajout");
            panier
                .ajouter(ligne("B", "1", &prix.to_string(), "5.5"))
                .expect("ajout");
            panier.remise_pct(remise).expect("remise valide");
            assert_eq!(
                panier.total_ttc(),
                panier.total_ht() + panier.total_tva(),
                "ecart au centime pour qte={quantite} prix={prix} remise={remise}"
            );
            quantite = arrondi_pourcentage(quantite + d("1.37"));
            prix = arrondi_centime(prix + d("3.91"));
            if remise < d("99") {
                remise = arrondi_pourcentage(remise + d("0.7"));
            }
        }
    }

    #[test]
    fn moyens_equivalents_et_especes_reconnues() {
        assert!(est_especes("especes"));
        assert!(est_especes("ESPECES"));
        assert!(est_especes(" especes "));
        assert!(est_especes("espèces"));
        assert!(est_especes("Cash"));
        assert!(!est_especes("cb"));
        assert!(!est_especes("cheque"));
        assert_eq!(normalise_moyen("  Espèces "), "especes");
    }

    #[test]
    fn erreur_affiche_le_contexte() {
        let erreur = Erreur::QuantiteInvalide {
            reference: "ART1".to_string(),
            quantite: Decimal::ZERO,
        };
        let texte = erreur.to_string();
        assert!(texte.contains("ART1"));
        assert!(texte.contains("quantite invalide"));

        // Le type implemente std::error::Error (utilisable en source d'erreur).
        let dynamique: &dyn std::error::Error = &erreur;
        assert!(dynamique.to_string().contains("quantite invalide"));
    }
}
