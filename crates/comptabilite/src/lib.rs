//! Moteur de partie double : le grand livre du noyau, tenu et verifie.
//!
//! Aujourd'hui le « grand livre » du noyau est **derive des documents** (devis,
//! factures, paiements) : on peut le relire, mais il n'est pas *tenu*. Une
//! ecriture fausse y entre sans que personne ne la refuse. Ce crate est l'autre
//! moitie : on lui donne des ecritures, il verifie qu'elles tiennent, il les
//! range par compte, il calcule les soldes et il apparie ce qui doit l'etre.
//!
//! # Ce que le moteur garantit
//!
//! - **Rien n'entre desequilibre** : le debit total doit egaler le credit total
//!   *au centime*. Une ecriture refusee ne laisse aucune trace dans le grand
//!   livre (ni mouvement, ni solde, ni numero consomme).
//! - **Une ligne porte un seul sens** : un debit **ou** un credit, jamais les
//!   deux, jamais un montant negatif. Le sens inverse s'ecrit avec une autre
//!   ligne, pas avec un signe.
//! - **Pas de double saisie** : le numero d'ecriture est l'identite de
//!   l'ecriture ; un second passage du meme numero est refuse.
//! - **La balance boucle** : par construction, `total_debit()` vaut
//!   `total_credit()` et la somme des soldes de la balance vaut zero. C'est le
//!   controle qui prouve que la partie double tient.
//! - **Montants exacts** : tout est en `Decimal` (`rust_decimal`), jamais en
//!   flottant. Le grand livre stocke les montants **tels qu'ils ont ete
//!   saisis** : on n'arrondit pas ligne par ligne a l'entree, sinon deux cotes
//!   arrondis separement pourraient casser l'egalite (0,004 + 0,004 = 0,01
//!   d'un cote contre 0,01 de l'autre arrondi en une fois : le grand livre
//!   deriverait). L'arrondi commercial au centime ne sert qu'a la
//!   *verification*.
//!
//! # Ce que ce crate ne fait pas
//!
//! C'est un **moteur**, pas une comptabilite. Il ne remplacera jamais un
//! logiciel de comptabilite, et il ne faut pas lui demander :
//!
//! - **aucun plan comptable** : un numero de compte est une chaine libre, le
//!   moteur ne sait pas ce qu'est un 512 ou un 401, ne verifie pas que le
//!   compte existe, ni qu'il est imputable, ni qu'il est ouvert ;
//! - **aucun journal reglementaire** : le champ `journal` est recopie tel quel.
//!   Pas de numerotation par journal, pas de controle de sequence, pas de date
//!   de verrouillage, pas de brouillard ;
//! - **aucun a-nouveau, aucun report a nouveau** : le grand livre demarre vide
//!   a chaque processus. La reprise des soldes d'ouverture est a la charge de
//!   l'appelant, qui doit les saisir comme des ecritures ;
//! - **aucune cloture** : pas d'exercice, pas de resultat, pas d'affectation,
//!   pas d'extourne automatique ;
//! - **aucune TVA** : ni collectee, ni deductible, ni declaration (CA3/CA12,
//!   autoliquidation, taux reduits). Une ligne de TVA est une ligne comme une
//!   autre, c'est-a-dire un compte a debiter ou a crediter ;
//! - **aucun rapprochement bancaire**, aucune devise etrangere, aucun axe
//!   analytique, aucune immobilisation, aucun amortissement, aucune lettre de
//!   change ;
//! - **aucune persistance** : rien n'est ecrit sur disque. Le stockage est a
//!   brancher au-dessus de ce crate.
//!
//! Le lettrage fourni est volontairement pauvre : c'est un appariement FIFO
//! debit/crdit sur un compte, pas une gestion des reglements partiels avec
//! ecarts de change. Il sert a repondre a une seule question : « ce compte
//! est-il solde, et sinon de combien ? ».

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt;

use rust_decimal::{Decimal, RoundingStrategy};

/// Nombre de decimales d'un montant en euros.
const SCALE_CENTIME: u32 = 2;

/// Arrondi commercial au centime : demi hors de zero.
///
/// C'est l'arrondi de `money::round_money`, donc celui de Dolibarr
/// (`price2num($x, 'MT')`) : 0,125 devient 0,13 et non 0,12.
pub fn arrondi_centime(montant: Decimal) -> Decimal {
    montant.round_dp_with_strategy(SCALE_CENTIME, RoundingStrategy::MidpointAwayFromZero)
}

/// Ce qui empeche une ecriture d'entrer dans le grand livre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Erreur {
    /// Le debit total ne vaut pas le credit total au centime.
    ///
    /// Les deux totaux sont arrondis au centime : l'ecart est chiffre, il ne
    /// reste pas a deviner.
    Desequilibre { debit: Decimal, credit: Decimal },
    /// Une ligne qui ne peut pas exister en partie double : deux sens a la
    /// fois, un montant negatif, ou aucun montant du tout.
    LigneInvalide(String),
    /// Numero de compte vide (ou fait uniquement d'espaces).
    CompteVide,
    /// Moins de deux lignes : il n'y a pas d'ecriture, et une ecriture sans
    /// numero n'a pas d'identite.
    EcritureVide,
    /// Ce numero d'ecriture est deja dans le grand livre : c'est un doublon.
    DejaAjoutee(String),
}

impl fmt::Display for Erreur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erreur::Desequilibre { debit, credit } => {
                let ecart = arrondi_centime(*debit) - arrondi_centime(*credit);
                write!(
                    f,
                    "ecriture desequilibree : debit {}, credit {}, ecart {}",
                    arrondi_centime(*debit),
                    arrondi_centime(*credit),
                    ecart
                )
            }
            Erreur::LigneInvalide(motif) => write!(f, "ligne invalide : {motif}"),
            Erreur::CompteVide => write!(f, "numero de compte vide"),
            Erreur::EcritureVide => {
                write!(
                    f,
                    "ecriture vide : il faut un numero et au moins deux lignes"
                )
            }
            Erreur::DejaAjoutee(numero) => write!(f, "ecriture deja ajoutee : {numero}"),
        }
    }
}

impl std::error::Error for Erreur {}

/// Une ligne de compte : un seul sens, un montant, un compte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ligne {
    /// Numero de compte, tel que saisi (le moteur ne connait aucun plan
    /// comptable : il ne verifie que la non-vacuite).
    pub compte: String,
    /// Libelle de l'ecriture sur cette ligne.
    pub libelle: String,
    /// Montant au debit, positif ou nul.
    pub debit: Decimal,
    /// Montant au credit, positif ou nul.
    pub credit: Decimal,
    /// Code journal, recopie tel quel (`VE`, `AC`, `BQ`...).
    pub journal: String,
    /// Date de l'operation, chaine libre. Une date vide prend la date de
    /// l'ecriture au moment de l'ajout au grand livre. Pour le lettrage FIFO,
    /// les formats `AAAA-MM-JJ` et `JJ/MM/AAAA` sont tries
    /// chronologiquement ; les autres formats sont tries apres, dans l'ordre
    /// d'ajout.
    pub date: String,
}

/// Une ecriture comptable : ce qui doit tenir pour entrer au grand livre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ecriture {
    /// Identite de l'ecriture. Sert de garde-fou contre la double saisie.
    pub numero: String,
    /// Date de l'ecriture (voir `Ligne::date` pour les formats tries).
    pub date: String,
    /// Libelle de l'ecriture, repris sur chaque ligne du grand livre.
    pub libelle: String,
    /// Les lignes, au moins deux, equilibrees.
    pub lignes: Vec<Ligne>,
}

/// Verifie qu'une ecriture tient, sans rien enregistrer.
///
/// Controles, dans cet ordre :
/// 1. l'ecriture porte un numero et au moins deux lignes (`EcritureVide`) ;
/// 2. chaque ligne a un compte non vide (`CompteVide`), un seul sens, et un
///    montant positif ou nul (`LigneInvalide`) ;
/// 3. le debit total egale le credit total au centime (`Desequilibre`).
pub fn verifier(ecriture: &Ecriture) -> Result<(), Erreur> {
    if ecriture.numero.trim().is_empty() || ecriture.lignes.len() < 2 {
        return Err(Erreur::EcritureVide);
    }

    let mut debit = Decimal::ZERO;
    let mut credit = Decimal::ZERO;
    for ligne in &ecriture.lignes {
        verifier_ligne(ligne)?;
        debit += ligne.debit;
        credit += ligne.credit;
    }

    let debit = arrondi_centime(debit);
    let credit = arrondi_centime(credit);
    if debit != credit {
        return Err(Erreur::Desequilibre { debit, credit });
    }
    Ok(())
}

/// Les controles d'une ligne : un compte, un seul sens, un montant positif.
fn verifier_ligne(ligne: &Ligne) -> Result<(), Erreur> {
    if ligne.compte.trim().is_empty() {
        return Err(Erreur::CompteVide);
    }
    if ligne.debit < Decimal::ZERO {
        return Err(Erreur::LigneInvalide(format!(
            "debit negatif ({}) sur le compte {}",
            ligne.debit, ligne.compte
        )));
    }
    if ligne.credit < Decimal::ZERO {
        return Err(Erreur::LigneInvalide(format!(
            "credit negatif ({}) sur le compte {}",
            ligne.credit, ligne.compte
        )));
    }
    if !ligne.debit.is_zero() && !ligne.credit.is_zero() {
        return Err(Erreur::LigneInvalide(format!(
            "debit et credit sur la meme ligne (compte {}) : une ligne porte un seul sens",
            ligne.compte
        )));
    }
    if ligne.debit.is_zero() && ligne.credit.is_zero() {
        return Err(Erreur::LigneInvalide(format!(
            "aucun montant sur le compte {} : une ligne porte un debit ou un credit",
            ligne.compte
        )));
    }
    Ok(())
}

/// Les mouvements d'un compte et leurs cumuls.
#[derive(Debug, Default)]
struct Compte {
    lignes: Vec<Ligne>,
    debit: Decimal,
    credit: Decimal,
}

/// Le grand livre : les mouvements ranges par compte, et les cumuls qui vont
/// avec.
///
/// Un compte est cree au premier mouvement, il n'y a pas de plan comptable a
/// alimenter. Les comptes sont tenus en ordre alphabetique (`BTreeMap`), donc
/// la balance sort deja triee.
#[derive(Debug, Default)]
pub struct GrandLivre {
    comptes: BTreeMap<String, Compte>,
    numeros: HashSet<String>,
}

impl GrandLivre {
    /// Un grand livre vide.
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Verifie l'ecriture puis l'enregistre.
    ///
    /// Une ecriture refusee ne modifie rien : le solde du grand livre est le
    /// meme avant et apres l'appel. Une ligne dont la date est vide prend la
    /// date de l'ecriture.
    pub fn ajouter(&mut self, ecriture: &Ecriture) -> Result<(), Erreur> {
        verifier(ecriture)?;
        if self.numeros.contains(&ecriture.numero) {
            return Err(Erreur::DejaAjoutee(ecriture.numero.clone()));
        }

        for ligne in &ecriture.lignes {
            let mut ligne = ligne.clone();
            // Une ligne sans date prend la date de son ecriture : c'est la date
            // de la ligne que le lettrage lit ensuite.
            if ligne.date.trim().is_empty() {
                ligne.date = ecriture.date.clone();
            }
            let compte = self.comptes.entry(ligne.compte.clone()).or_default();
            compte.debit += ligne.debit;
            compte.credit += ligne.credit;
            compte.lignes.push(ligne);
        }
        self.numeros.insert(ecriture.numero.clone());
        Ok(())
    }

    /// Solde d'un compte : debit moins credit. Un compte inconnu vaut zero.
    ///
    /// Signe : positif = compte debiteur, negatif = compte crediteur.
    pub fn solde(&self, compte: &str) -> Decimal {
        match self.comptes.get(compte) {
            Some(c) => c.debit - c.credit,
            None => Decimal::ZERO,
        }
    }

    /// Les mouvements d'un compte, dans l'ordre d'ajout au grand livre.
    ///
    /// Un compte inconnu rend une tranche vide (pas d'erreur : demander les
    /// mouvements d'un compte qui n'a rien vecu n'est pas une faute).
    pub fn mouvements(&self, compte: &str) -> &[Ligne] {
        match self.comptes.get(compte) {
            Some(c) => &c.lignes,
            None => &[],
        }
    }

    /// La balance : `(compte, total debit, total credit)`, triee par compte.
    pub fn balance(&self) -> Vec<(String, Decimal, Decimal)> {
        self.comptes
            .iter()
            .map(|(nom, c)| (nom.clone(), c.debit, c.credit))
            .collect()
    }

    /// Somme des debits de tous les comptes.
    pub fn total_debit(&self) -> Decimal {
        self.comptes.values().map(|c| c.debit).sum()
    }

    /// Somme des credits de tous les comptes.
    pub fn total_credit(&self) -> Decimal {
        self.comptes.values().map(|c| c.credit).sum()
    }
}

/// Un debit et un credit rapproches, pour le montant qu'ils ont en commun.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paire {
    /// La ligne debit d'origine.
    pub debit: Ligne,
    /// La ligne credit d'origine.
    pub credit: Ligne,
    /// Le montant apparié (le plus petit des deux restes).
    pub montant: Decimal,
}

/// Le lettrage d'un compte : les paires soldees et ce qui reste ouvert.
///
/// Le lettrage est un appariement **FIFO** : le debit le plus ancien mange le
/// credit le plus ancien, et ainsi de suite. C'est le lettrage d'un comptable
/// presse, pas celui d'un logiciel avec des regles de rapprochement.
#[derive(Debug, Default)]
pub struct Lettrage {
    paires: HashMap<String, Vec<Paire>>,
    restes: HashMap<String, Vec<Ligne>>,
}

impl Lettrage {
    /// Un lettrage vide.
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Apparie les debits et les credits d'un compte, du plus ancien au plus
    /// recent, et rend le nombre de paires soldees.
    ///
    /// Un appel remplace le lettrage precedent de ce compte : appeler deux fois
    /// ne double rien. Un compte inconnu rend `Ok(0)`.
    pub fn lettrer(&mut self, grand_livre: &GrandLivre, compte: &str) -> Result<usize, Erreur> {
        if compte.trim().is_empty() {
            return Err(Erreur::CompteVide);
        }

        let mouvements = grand_livre.mouvements(compte);

        // Ordre FIFO : par date croissante, puis par ordre d'ajout (le tri est
        // stable, l'indice sert d'arbitre).
        let mut ordre: Vec<usize> = (0..mouvements.len()).collect();
        ordre.sort_by_key(|&i| (cle_de_date(&mouvements[i].date), i));

        let mut debits: VecDeque<(usize, Decimal)> = VecDeque::new();
        let mut credits: VecDeque<(usize, Decimal)> = VecDeque::new();
        for &i in &ordre {
            let ligne = &mouvements[i];
            if !ligne.debit.is_zero() {
                debits.push_back((i, ligne.debit));
            }
            if !ligne.credit.is_zero() {
                credits.push_back((i, ligne.credit));
            }
        }

        let mut paires: Vec<Paire> = Vec::new();
        loop {
            // On copie les tetes de file : l'emprunt s'arrete ici, on peut donc
            // modifier les files juste apres.
            let tete_debit = debits.front().copied();
            let tete_credit = credits.front().copied();
            let (Some((index_debit, reste_debit)), Some((index_credit, reste_credit))) =
                (tete_debit, tete_credit)
            else {
                break;
            };

            let montant = if reste_debit < reste_credit {
                reste_debit
            } else {
                reste_credit
            };

            paires.push(Paire {
                debit: mouvements[index_debit].clone(),
                credit: mouvements[index_credit].clone(),
                montant,
            });

            let nouveau_reste_debit = reste_debit - montant;
            let nouveau_reste_credit = reste_credit - montant;
            if nouveau_reste_debit.is_zero() {
                debits.pop_front();
            } else if let Some(tete) = debits.front_mut() {
                tete.1 = nouveau_reste_debit;
            }
            if nouveau_reste_credit.is_zero() {
                credits.pop_front();
            } else if let Some(tete) = credits.front_mut() {
                tete.1 = nouveau_reste_credit;
            }
        }

        // Ce qui n'a pas ete apparie reste visible, avec le montant restant.
        let mut restes: Vec<Ligne> = Vec::new();
        for &(index, montant) in &debits {
            let mut ligne = mouvements[index].clone();
            ligne.debit = montant;
            ligne.credit = Decimal::ZERO;
            restes.push(ligne);
        }
        for &(index, montant) in &credits {
            let mut ligne = mouvements[index].clone();
            ligne.debit = Decimal::ZERO;
            ligne.credit = montant;
            restes.push(ligne);
        }

        let nombre = paires.len();
        self.paires.insert(compte.to_string(), paires);
        self.restes.insert(compte.to_string(), restes);
        Ok(nombre)
    }

    /// Les paires soldees d'un compte, dans l'ordre du lettrage.
    pub fn paires(&self, compte: &str) -> &[Paire] {
        match self.paires.get(compte) {
            Some(p) => p,
            None => &[],
        }
    }

    /// Les mouvements non apparies d'un compte, avec leur montant restant.
    pub fn restes(&self, compte: &str) -> &[Ligne] {
        match self.restes.get(compte) {
            Some(r) => r,
            None => &[],
        }
    }

    /// Ce qui reste ouvert sur un compte : debit restant moins credit restant.
    ///
    /// Zero veut dire que le compte est solde. C'est le meme solde que
    /// `GrandLivre::solde`, calcule autrement : les deux doivent concorder.
    pub fn solde_restant(&self, compte: &str) -> Decimal {
        let mut solde = Decimal::ZERO;
        for ligne in self.restes(compte) {
            solde += ligne.debit - ligne.credit;
        }
        solde
    }
}

/// Cle de tri chronologique d'une date.
///
/// Formats reconnus : `AAAA-MM-JJ`, `AAAA/MM/JJ`, `JJ/MM/AAAA`, `JJ-MM-AAAA`,
/// et `JJ/MM` (sans annee, considere comme le plus ancien). Tout le reste est
/// place apres, dans l'ordre d'ajout : mieux vaut un ordre d'ajout qu'un ordre
/// invente.
fn cle_de_date(date: &str) -> (i64, u32, u32) {
    match analyser_date(date) {
        Some((annee, mois, jour)) => (annee, mois, jour),
        None => (i64::MAX, 0, 0),
    }
}

/// `(annee, mois, jour)` si la date est dans un format reconnu.
fn analyser_date(date: &str) -> Option<(i64, u32, u32)> {
    let texte = date.trim();
    let separateur = if texte.contains('-') {
        '-'
    } else if texte.contains('/') {
        '/'
    } else {
        return None;
    };
    let morceaux: Vec<&str> = texte.split(separateur).collect();

    let (annee, mois, jour) = match morceaux.as_slice() {
        [a, m, j] => {
            if a.len() == 4 {
                (
                    a.parse::<i64>().ok()?,
                    m.parse::<u32>().ok()?,
                    j.parse::<u32>().ok()?,
                )
            } else {
                (
                    j.parse::<i64>().ok()?,
                    m.parse::<u32>().ok()?,
                    a.parse::<u32>().ok()?,
                )
            }
        }
        [j, m] => (0, m.parse::<u32>().ok()?, j.parse::<u32>().ok()?),
        _ => return None,
    };

    if annee < 0 || !(1..=12).contains(&mois) || !(1..=31).contains(&jour) {
        return None;
    }
    Some((annee, mois, jour))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(valeur: &str) -> Decimal {
        Decimal::from_str(valeur).expect("montant de test illisible")
    }

    fn ligne(compte: &str, debit: &str, credit: &str) -> Ligne {
        Ligne {
            compte: compte.to_string(),
            libelle: "libelle de test".to_string(),
            debit: d(debit),
            credit: d(credit),
            journal: "VE".to_string(),
            date: "2026-01-01".to_string(),
        }
    }

    fn ecriture(numero: &str, lignes: Vec<Ligne>) -> Ecriture {
        Ecriture {
            numero: numero.to_string(),
            date: "2026-01-01".to_string(),
            libelle: "ecriture de test".to_string(),
            lignes,
        }
    }

    /// Une ecriture dont la date est posee partout, y compris sur les lignes :
    /// c'est la date de la ligne que le lettrage lit.
    fn ecriture_datee(numero: &str, date: &str, lignes: Vec<Ligne>) -> Ecriture {
        let lignes = lignes
            .into_iter()
            .map(|l| Ligne {
                date: date.to_string(),
                ..l
            })
            .collect();
        Ecriture {
            date: date.to_string(),
            ..ecriture(numero, lignes)
        }
    }

    /// Vente de prestation : banque au debit, produit au credit.
    fn vente(numero: &str, montant: &str) -> Ecriture {
        ecriture(
            numero,
            vec![ligne("512", montant, "0"), ligne("706", "0", montant)],
        )
    }

    // 1. Equilibre au centime --------------------------------------------------

    #[test]
    fn l_equilibre_est_verifie_au_centime() {
        // 100,00 + 50,00 d'un cote, 150,00 de l'autre.
        let e = ecriture(
            "E1",
            vec![
                ligne("512", "100.00", "0"),
                ligne("512", "50.00", "0"),
                ligne("706", "0", "150.00"),
            ],
        );
        assert_eq!(verifier(&e), Ok(()));

        // Le meme a un centime pres : 150,01 en face de 150,00.
        let faux = ecriture(
            "E2",
            vec![
                ligne("512", "100.00", "0"),
                ligne("512", "50.00", "0"),
                ligne("706", "0", "150.01"),
            ],
        );
        assert_eq!(
            verifier(&faux),
            Err(Erreur::Desequilibre {
                debit: d("150.00"),
                credit: d("150.01"),
            })
        );
    }

    #[test]
    fn le_desequilibre_est_chiffre_dans_le_message() {
        let faux = ecriture(
            "E1",
            vec![ligne("512", "150.00", "0"), ligne("706", "0", "150.01")],
        );
        let erreur = verifier(&faux).expect_err("150,00 contre 150,01 doit etre refuse");
        let texte = erreur.to_string();
        assert!(texte.contains("150.00"), "message : {texte}");
        assert!(texte.contains("150.01"), "message : {texte}");
        assert!(texte.contains("-0.01"), "message : {texte}");
    }

    #[test]
    fn l_ecart_de_centime_ne_fait_pas_deriver_le_grand_livre() {
        // Deux ecritures a trois lignes, dont les arrondis se compensent : le
        // grand livre doit rester exactement a zero, sans arrondi intermediaire.
        let mut gl = GrandLivre::nouveau();
        let e1 = ecriture(
            "E1",
            vec![
                ligne("606", "33.33", "0"),
                ligne("606", "33.33", "0"),
                ligne("606", "33.34", "0"),
                ligne("401", "0", "100.00"),
            ],
        );
        gl.ajouter(&e1).expect("ecriture equilibree");
        let e2 = ecriture(
            "E2",
            vec![
                ligne("401", "100.00", "0"),
                ligne("512", "0", "99.99"),
                ligne("512", "0", "0.01"),
            ],
        );
        gl.ajouter(&e2).expect("ecriture equilibree");

        assert_eq!(gl.solde("606"), d("100.00"));
        assert_eq!(gl.solde("401"), Decimal::ZERO);
        assert_eq!(gl.solde("512"), d("-100.00"));
        assert_eq!(gl.total_debit(), gl.total_credit());
    }

    // 2. Une ligne, un sens ----------------------------------------------------

    #[test]
    fn une_ligne_porte_un_seul_sens() {
        let deux_sens = ecriture(
            "E1",
            vec![
                ligne("512", "100.00", "100.00"),
                ligne("706", "0", "100.00"),
            ],
        );
        match verifier(&deux_sens) {
            Err(Erreur::LigneInvalide(motif)) => {
                assert!(motif.contains("un seul sens"), "motif : {motif}");
            }
            autre => panic!("debit et credit sur la meme ligne : {autre:?}"),
        }
    }

    #[test]
    fn un_montant_negatif_est_refuse() {
        let debit_negatif = ecriture(
            "E1",
            vec![ligne("512", "-100.00", "0"), ligne("706", "0", "100.00")],
        );
        assert!(matches!(
            verifier(&debit_negatif),
            Err(Erreur::LigneInvalide(_))
        ));

        // Un credit negatif est un debit deguise : meme refus.
        let credit_negatif = ecriture(
            "E1",
            vec![ligne("512", "100.00", "0"), ligne("706", "0", "-100.00")],
        );
        assert!(matches!(
            verifier(&credit_negatif),
            Err(Erreur::LigneInvalide(_))
        ));
    }

    #[test]
    fn une_ligne_sans_montant_est_refusee() {
        // Zero des deux cotes : la ligne ne porte ni debit ni credit.
        let vide = ecriture(
            "E1",
            vec![
                ligne("512", "100.00", "0"),
                ligne("706", "0", "100.00"),
                ligne("401", "0", "0"),
            ],
        );
        assert!(matches!(verifier(&vide), Err(Erreur::LigneInvalide(_))));
    }

    #[test]
    fn un_compte_vide_est_refuse() {
        let sans_compte = ecriture(
            "E1",
            vec![ligne("   ", "100.00", "0"), ligne("706", "0", "100.00")],
        );
        assert_eq!(verifier(&sans_compte), Err(Erreur::CompteVide));
    }

    #[test]
    fn une_ecriture_d_une_seule_ligne_est_refusee() {
        let seule = ecriture("E1", vec![ligne("512", "0", "0")]);
        assert_eq!(verifier(&seule), Err(Erreur::EcritureVide));

        let sans_numero = Ecriture {
            numero: "  ".to_string(),
            ..ecriture(
                "E1",
                vec![ligne("512", "100.00", "0"), ligne("706", "0", "100.00")],
            )
        };
        assert_eq!(verifier(&sans_numero), Err(Erreur::EcritureVide));
    }

    // 3. Refus a l'ajout ------------------------------------------------------

    #[test]
    fn une_ecriture_desequilibree_n_entre_pas_dans_le_grand_livre() {
        let mut gl = GrandLivre::nouveau();
        gl.ajouter(&vente("E1", "100.00"))
            .expect("vente equilibree");
        let solde_avant = gl.solde("512");
        let debit_avant = gl.total_debit();

        let fausse = ecriture(
            "E2",
            vec![ligne("512", "150.00", "0"), ligne("706", "0", "149.99")],
        );
        assert!(matches!(
            gl.ajouter(&fausse),
            Err(Erreur::Desequilibre { .. })
        ));

        assert_eq!(gl.solde("512"), solde_avant);
        assert_eq!(gl.total_debit(), debit_avant);
        assert!(gl.mouvements("512").len() == 1, "aucun mouvement ajoute");
        // Le numero refuse n'est pas consomme : l'ecriture corrigee passe.
        let corrigee = ecriture(
            "E2",
            vec![ligne("512", "150.00", "0"), ligne("706", "0", "150.00")],
        );
        gl.ajouter(&corrigee).expect("ecriture corrigee");
    }

    // 4. Double saisie --------------------------------------------------------

    #[test]
    fn deux_ecritures_de_meme_numero_sont_refusees() {
        let mut gl = GrandLivre::nouveau();
        gl.ajouter(&vente("VE-2026-0001", "100.00"))
            .expect("premiere saisie");
        let erreur = gl
            .ajouter(&vente("VE-2026-0001", "100.00"))
            .expect_err("le meme numero ne passe pas deux fois");
        assert_eq!(erreur, Erreur::DejaAjoutee("VE-2026-0001".to_string()));
        // Le refus n'a rien ecrit : le solde n'a pas double.
        assert_eq!(gl.solde("512"), d("100.00"));
        assert_eq!(gl.mouvements("512").len(), 1);
    }

    // 5. La balance juste -----------------------------------------------------

    #[test]
    fn la_balance_vaut_zero_sur_trois_ecritures() {
        let mut gl = GrandLivre::nouveau();
        // Vente 1 200 : banque au debit, produit au credit.
        gl.ajouter(&ecriture_datee(
            "VE-1",
            "2026-01-10",
            vec![ligne("512", "1200.00", "0"), ligne("706", "0", "1200.00")],
        ))
        .expect("vente");
        // Achat 300 : charge au debit, fournisseur au credit.
        gl.ajouter(&ecriture_datee(
            "AC-1",
            "2026-01-15",
            vec![ligne("606", "300.00", "0"), ligne("401", "0", "300.00")],
        ))
        .expect("achat");
        // Reglement du fournisseur : fournisseur au debit, banque au credit.
        gl.ajouter(&ecriture_datee(
            "BQ-1",
            "2026-01-20",
            vec![ligne("401", "300.00", "0"), ligne("512", "0", "300.00")],
        ))
        .expect("reglement");

        assert_eq!(gl.total_debit(), d("1800.00"));
        assert_eq!(gl.total_credit(), d("1800.00"));
        assert_eq!(gl.total_debit(), gl.total_credit());

        let balance = gl.balance();
        let somme_des_soldes: Decimal = balance.iter().map(|(_, d, c)| d - c).sum();
        assert_eq!(
            somme_des_soldes,
            Decimal::ZERO,
            "la somme des soldes de la balance doit valoir zero : {balance:?}"
        );

        // Les soldes attendus, compte par compte.
        assert_eq!(gl.solde("512"), d("900.00"));
        assert_eq!(gl.solde("706"), d("-1200.00"));
        assert_eq!(gl.solde("606"), d("300.00"));
        assert_eq!(gl.solde("401"), Decimal::ZERO);
        assert_eq!(gl.solde("999"), Decimal::ZERO, "compte inconnu");
    }

    #[test]
    fn la_balance_est_triee_par_compte() {
        let mut gl = GrandLivre::nouveau();
        gl.ajouter(&vente("E1", "100.00")).expect("vente");
        gl.ajouter(&ecriture(
            "E2",
            vec![ligne("401", "10.00", "0"), ligne("606", "0", "10.00")],
        ))
        .expect("achat");

        let comptes: Vec<String> = gl.balance().into_iter().map(|(c, _, _)| c).collect();
        assert_eq!(comptes, vec!["401", "512", "606", "706"]);

        let mut trie = comptes.clone();
        trie.sort();
        assert_eq!(comptes, trie, "la balance sort triee");
    }

    #[test]
    fn les_mouvements_d_un_compte_suivent_l_ordre_d_ajout() {
        let mut gl = GrandLivre::nouveau();
        gl.ajouter(&vente("E1", "100.00")).expect("vente");
        gl.ajouter(&ecriture(
            "E2",
            vec![ligne("512", "0", "30.00"), ligne("401", "30.00", "0")],
        ))
        .expect("reglement partiel");

        let mouvements = gl.mouvements("512");
        assert_eq!(mouvements.len(), 2);
        assert_eq!(mouvements[0].debit, d("100.00"));
        assert_eq!(mouvements[1].credit, d("30.00"));
        assert_eq!(gl.solde("512"), d("70.00"));
        assert!(gl.mouvements("000").is_empty(), "compte inconnu");
    }

    // 6. Lettrage FIFO --------------------------------------------------------

    #[test]
    fn le_lettrage_fifo_solde_deux_paires() {
        // Debit 100 le 01/01, credit 60 le 05/01, credit 40 le 10/01.
        let mut gl = GrandLivre::nouveau();
        gl.ajouter(&ecriture_datee(
            "E1",
            "01/01/2026",
            vec![ligne("411", "100.00", "0"), ligne("701", "0", "100.00")],
        ))
        .expect("facture");
        gl.ajouter(&ecriture_datee(
            "E2",
            "05/01/2026",
            vec![ligne("512", "60.00", "0"), ligne("411", "0", "60.00")],
        ))
        .expect("premier reglement");
        gl.ajouter(&ecriture_datee(
            "E3",
            "10/01/2026",
            vec![ligne("512", "40.00", "0"), ligne("411", "0", "40.00")],
        ))
        .expect("second reglement");

        let mut lettrage = Lettrage::nouveau();
        let paires = lettrage.lettrer(&gl, "411").expect("lettrage possible");
        assert_eq!(paires, 2, "100 mange 60 puis 40");

        assert_eq!(lettrage.restes("411").len(), 0, "rien ne reste");
        assert_eq!(lettrage.solde_restant("411"), Decimal::ZERO);

        let detail = lettrage.paires("411");
        assert_eq!(detail[0].montant, d("60.00"));
        assert_eq!(detail[0].debit.date, "01/01/2026");
        assert_eq!(detail[0].credit.date, "05/01/2026");
        assert_eq!(detail[1].montant, d("40.00"));
        assert_eq!(detail[1].credit.date, "10/01/2026");
    }

    #[test]
    fn le_lettrage_laisse_visible_ce_qui_reste_ouvert() {
        let mut gl = GrandLivre::nouveau();
        gl.ajouter(&ecriture_datee(
            "E1",
            "2026-01-01",
            vec![ligne("411", "100.00", "0"), ligne("701", "0", "100.00")],
        ))
        .expect("facture");
        gl.ajouter(&ecriture_datee(
            "E2",
            "2026-01-05",
            vec![ligne("512", "60.00", "0"), ligne("411", "0", "60.00")],
        ))
        .expect("reglement partiel");

        let mut lettrage = Lettrage::nouveau();
        assert_eq!(lettrage.lettrer(&gl, "411").expect("lettrage"), 1);

        let restes = lettrage.restes("411");
        assert_eq!(restes.len(), 1);
        assert_eq!(restes[0].debit, d("40.00"), "il reste 40 au debit");
        assert_eq!(restes[0].credit, Decimal::ZERO);
        assert_eq!(lettrage.solde_restant("411"), d("40.00"));
        // Le solde du grand livre et le solde restant disent la meme chose.
        assert_eq!(lettrage.solde_restant("411"), gl.solde("411"));
    }

    #[test]
    fn le_lettrage_travaille_du_plus_ancien_au_plus_recent() {
        // Saisie dans le desordre : le credit du 02/01 doit manger le debit du
        // 01/01, pas celui du 03/01.
        let mut gl = GrandLivre::nouveau();
        gl.ajouter(&ecriture_datee(
            "E3",
            "2026-03-01",
            vec![ligne("411", "80.00", "0"), ligne("701", "0", "80.00")],
        ))
        .expect("facture tardive");
        gl.ajouter(&ecriture_datee(
            "E1",
            "2026-01-01",
            vec![ligne("411", "50.00", "0"), ligne("701", "0", "50.00")],
        ))
        .expect("facture ancienne");
        gl.ajouter(&ecriture_datee(
            "E2",
            "2026-02-01",
            vec![ligne("512", "50.00", "0"), ligne("411", "0", "50.00")],
        ))
        .expect("reglement");

        let mut lettrage = Lettrage::nouveau();
        assert_eq!(lettrage.lettrer(&gl, "411").expect("lettrage"), 1);
        let restes = lettrage.restes("411");
        assert_eq!(restes.len(), 1, "il reste la facture de mars : {restes:?}");
        assert_eq!(restes[0].debit, d("80.00"));
        assert_eq!(restes[0].date, "2026-03-01");
    }

    #[test]
    fn un_deuxieme_lettrage_remplace_le_premier() {
        let mut gl = GrandLivre::nouveau();
        gl.ajouter(&vente("E1", "100.00")).expect("vente");
        gl.ajouter(&ecriture(
            "E2",
            vec![ligne("512", "0", "100.00"), ligne("401", "100.00", "0")],
        ))
        .expect("encaissement");

        let mut lettrage = Lettrage::nouveau();
        assert_eq!(lettrage.lettrer(&gl, "512").expect("lettrage"), 1);
        assert_eq!(lettrage.lettrer(&gl, "512").expect("lettrage"), 1);
        assert_eq!(lettrage.paires("512").len(), 1, "pas de doublon");
        assert!(lettrage.restes("512").is_empty());
    }

    #[test]
    fn le_lettrage_d_un_compte_inconnu_rend_zero() {
        let gl = GrandLivre::nouveau();
        let mut lettrage = Lettrage::nouveau();
        assert_eq!(lettrage.lettrer(&gl, "411").expect("compte vide"), 0);
        assert!(lettrage.paires("411").is_empty());
        assert!(lettrage.restes("411").is_empty());
        assert_eq!(lettrage.solde_restant("411"), Decimal::ZERO);
        assert_eq!(
            lettrage.lettrer(&gl, "   "),
            Err(Erreur::CompteVide),
            "un compte vide n'est pas un compte"
        );
    }

    // Divers -----------------------------------------------------------------

    #[test]
    fn les_erreurs_s_expliquent_en_francais() {
        let cas = [
            Erreur::Desequilibre {
                debit: d("150.00"),
                credit: d("150.01"),
            },
            Erreur::LigneInvalide("deux sens".to_string()),
            Erreur::CompteVide,
            Erreur::EcritureVide,
            Erreur::DejaAjoutee("VE-1".to_string()),
        ];
        for erreur in cas {
            let texte = erreur.to_string();
            assert!(!texte.is_empty());
            assert!(
                texte.chars().any(|c| c.is_ascii_lowercase()),
                "message en clair : {texte}"
            );
        }

        // L'erreur est une erreur standard : elle se propage avec `?`.
        fn remonter() -> Result<(), Box<dyn std::error::Error>> {
            verifier(&ecriture(
                "E1",
                vec![ligne("512", "1.00", "0"), ligne("706", "0", "2.00")],
            ))?;
            Ok(())
        }
        assert!(remonter().is_err());
    }

    #[test]
    fn une_date_illisible_ne_fait_pas_paniquer_le_lettrage() {
        let mut gl = GrandLivre::nouveau();
        let mut e1 = ecriture(
            "E1",
            vec![ligne("411", "10.00", "0"), ligne("701", "0", "10.00")],
        );
        e1.date = "pas une date".to_string();
        e1.lignes[0].date = "pas une date".to_string();
        gl.ajouter(&e1).expect("ecriture equilibree, date libre");
        let mut e2 = ecriture(
            "E2",
            vec![ligne("512", "10.00", "0"), ligne("411", "0", "10.00")],
        );
        e2.lignes[1].date = String::new();
        gl.ajouter(&e2).expect("ecriture equilibree, date vide");

        let mut lettrage = Lettrage::nouveau();
        assert_eq!(lettrage.lettrer(&gl, "411").expect("lettrage"), 1);
        assert_eq!(lettrage.solde_restant("411"), Decimal::ZERO);
        // La ligne sans date a herite de la date de son ecriture.
        let mouvements = gl.mouvements("411");
        assert_eq!(mouvements[1].date, "2026-01-01");
    }
}
