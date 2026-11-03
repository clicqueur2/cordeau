//! Recurrences : devis, factures, contrats et abonnements qui se regenerent.
//!
//! Dolibarr a `modRecurring` ; ce module decide **quand** le document suivant
//! doit etre emis. Il ne genere rien lui-meme : il repond a la seule question
//! qui compte avant d'emettre, et il y repond sans base de donnees et sans
//! horloge. C'est ce qui rend la regle verifiable : une date d'entree, une date
//! de sortie, aucun effet de bord.
//!
//! # Le vrai probleme : les fins de mois
//!
//! Un abonnement du 31 janvier doit tomber le **28 fevrier**, pas le 3 mars.
//! Ajouter « un mois » a un `datetime` ou 31 jours a un compteur donne le
//! 3 mars, et la facturation se decale d'un mois entier sans que personne ne
//! s'en apercoive. Ici la regle porte un [`Regle::jour`] : on avance de mois en
//! mois sur le **mois d'ancrage** (celui du debut), puis on ramene le jour
//! voulu au dernier jour du mois quand il n'existe pas. 31 janvier ->
//! 28 fevrier 2026 -> 31 mars, et 30 janvier -> 28 fevrier -> 30 mars.
//!
//! Le mois d'ancrage ne glisse jamais : une serie trimestrielle partie du
//! 31 janvier reste sur janvier, avril, juillet, octobre, meme si fevrier l'a
//! ramenee au 28.
//!
//! # Le calendrier
//!
//! Aucune dependance de calendrier. La conversion jours <-> date est celle de
//! Howard Hinnant (`days_from_civil` et `civil_from_days`), exacte pour toutes
//! les annees, sans table ni cas particulier. Une date circule en texte
//! `AAAA-MM-JJ`, zero de remplissage compris : c'est le format que Dolibarr
//! stocke, et l'ordre alphabetique y est l'ordre chronologique.
//!
//! # Exemple
//!
//! ```
//! use recurrence::{echeances, Frequence, Regle};
//!
//! # fn main() -> Result<(), recurrence::Erreur> {
//! let regle = Regle {
//!     frequence: Frequence::Mensuelle,
//!     debut: "2026-01-31".to_string(),
//!     fin: None,
//!     limite_occurrences: None,
//!     jour: 31,
//! };
//! assert_eq!(echeances(&regle, "2026-03-31", 10)?, ["2026-01-31", "2026-02-28", "2026-03-31"]);
//! # Ok(())
//! # }
//! ```

use std::fmt;

/// Le pas d'une recurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frequence {
    /// Tous les mois.
    Mensuelle,
    /// Tous les trois mois.
    Trimestrielle,
    /// Tous les six mois.
    Semestrielle,
    /// Tous les douze mois.
    Annuelle,
}

impl Frequence {
    /// Nombre de mois entre deux occurrences consecutives.
    #[must_use]
    pub fn pas_en_mois(self) -> u32 {
        match self {
            Frequence::Mensuelle => 1,
            Frequence::Trimestrielle => 3,
            Frequence::Semestrielle => 6,
            Frequence::Annuelle => 12,
        }
    }

    /// Libelle pour un ecran de saisie.
    #[must_use]
    pub fn libelle(self) -> &'static str {
        match self {
            Frequence::Mensuelle => "Mensuelle",
            Frequence::Trimestrielle => "Trimestrielle",
            Frequence::Semestrielle => "Semestrielle",
            Frequence::Annuelle => "Annuelle",
        }
    }
}

/// Ce qui empeche de calculer les echeances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Erreur {
    /// Le texte recu n'est pas une date `AAAA-MM-JJ` qui existe au calendrier.
    DateInvalide(String),
    /// La regle se contredit : fin avant le debut, limite nulle.
    RegleInvalide(String),
    /// Le jour du mois demande n'est pas dans `1..=31`.
    JourInvalide(u32),
}

impl fmt::Display for Erreur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erreur::DateInvalide(texte) => write!(f, "date invalide : {texte}"),
            Erreur::RegleInvalide(detail) => write!(f, "regle invalide : {detail}"),
            Erreur::JourInvalide(jour) => write!(f, "jour invalide : {jour}"),
        }
    }
}

impl std::error::Error for Erreur {}

/// Une regle de recurrence : ce que l'ecran de saisie d'un contrat enregistre.
///
/// `debut` est la **premiere echeance**, reprise telle quelle : c'est la date
/// portee sur le premier document, celle que le client a signee. Les suivantes
/// sont calculees depuis le mois de `debut`, en retombant sur `jour`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regle {
    /// Le pas de la serie.
    pub frequence: Frequence,
    /// Premiere echeance, `AAAA-MM-JJ`.
    pub debut: String,
    /// Derniere echeance admise, incluse. `None` : la serie n'a pas de fin.
    pub fin: Option<String>,
    /// Nombre total d'echeances admises, `debut` compris. `None` : illimite.
    pub limite_occurrences: Option<u32>,
    /// Jour du mois voulu. 31 vaut « dernier jour du mois » quand le mois est
    /// plus court (28 ou 29 en fevrier, 30 en avril).
    pub jour: u32,
}

impl Regle {
    /// Verifie la regle et rend le debut et la fin deja convertis.
    fn verifier(&self) -> Result<(Date, Option<Date>), Erreur> {
        if self.jour == 0 || self.jour > 31 {
            return Err(Erreur::JourInvalide(self.jour));
        }
        let debut = Date::analyser(&self.debut)?;
        let fin = match &self.fin {
            Some(texte) => Some(Date::analyser(texte)?),
            None => None,
        };
        if let Some(fin) = fin {
            if fin < debut {
                return Err(Erreur::RegleInvalide(format!(
                    "fin {} anterieure au debut {}",
                    fin.texte(),
                    debut.texte()
                )));
            }
        }
        if self.limite_occurrences == Some(0) {
            return Err(Erreur::RegleInvalide(
                "limite d occurrences nulle : la serie ne produirait rien".to_string(),
            ));
        }
        Ok((debut, fin))
    }
}

/// La date suivante de la serie, **strictement posterieure** a `apres`.
///
/// Rend `None` quand la regle est finie : `fin` depassee, ou
/// [`Regle::limite_occurrences`] atteinte. Si `apres` precede le debut, la
/// premiere echeance est rendue : c'est la prochaine date a honorer.
///
/// # Erreurs
///
/// [`Erreur::DateInvalide`] si `apres` n'est pas une date `AAAA-MM-JJ` valide,
/// [`Erreur::RegleInvalide`] ou [`Erreur::JourInvalide`] si la regle elle-meme
/// ne tient pas debout.
pub fn prochaine(regle: &Regle, apres: &str) -> Result<Option<String>, Erreur> {
    let (debut, fin) = regle.verifier()?;
    let apres = Date::analyser(apres)?;
    let limite = regle.limite_occurrences.map(u64::from);
    let mut indice = indice_de_depart(debut, apres, regle.frequence.pas_en_mois());
    loop {
        if limite.is_some_and(|l| indice >= l) {
            return Ok(None);
        }
        let Some(candidat) = occurrence(regle, debut, indice) else {
            return Ok(None);
        };
        if fin.is_some_and(|f| candidat > f) {
            return Ok(None);
        }
        if candidat > apres {
            return Ok(Some(candidat.texte()));
        }
        indice = indice.saturating_add(1);
    }
}

/// Toutes les echeances de `debut` a `jusqu_a`, **incluses**, dans l'ordre.
///
/// La serie s'arrete a la premiere des bornes atteintes : `jusqu_a`,
/// [`Regle::fin`], [`Regle::limite_occurrences`] ou `max` elements. `max` a `0`
/// rend une liste vide sans rien calculer.
///
/// # Erreurs
///
/// [`Erreur::DateInvalide`] si l'une des deux dates est mal formee,
/// [`Erreur::RegleInvalide`] ou [`Erreur::JourInvalide`] si la regle est
/// incoherente.
pub fn echeances(regle: &Regle, jusqu_a: &str, max: usize) -> Result<Vec<String>, Erreur> {
    let jusqu_a = Date::analyser(jusqu_a)?;
    let mut dates = Vec::new();
    parcours(regle, jusqu_a, Some(max), |date| {
        dates.push(date.texte());
        true
    })?;
    Ok(dates)
}

/// Combien d'echeances tombent entre `debut` et `jusqu_a`, bornes de la regle
/// comprises.
///
/// C'est le nombre de documents a emettre pour honorer la serie jusqu'a cette
/// date : `debut` compte pour un, et `fin` comme
/// [`Regle::limite_occurrences`] tronquent le total.
///
/// # Erreurs
///
/// Les memes que [`echeances`].
pub fn nombre_restant(regle: &Regle, jusqu_a: &str) -> Result<usize, Erreur> {
    let jusqu_a = Date::analyser(jusqu_a)?;
    let mut total = 0usize;
    parcours(regle, jusqu_a, None, |_| {
        total = total.saturating_add(1);
        true
    })?;
    Ok(total)
}

/// Visite les echeances de `debut` a `jusqu_a` comprises.
///
/// `visite` rend `false` pour interrompre. Le parcours s'arrete aussi des que
/// la date sort de l'an 1..9999 : au-dela, il n'y a plus de date a formater, et
/// continuer ne produirait qu'un depassement d'entier.
fn parcours(
    regle: &Regle,
    jusqu_a: Date,
    max: Option<usize>,
    mut visite: impl FnMut(Date) -> bool,
) -> Result<(), Erreur> {
    let (debut, fin) = regle.verifier()?;
    let limite = regle.limite_occurrences.map(u64::from);
    let mut indice: u64 = 0;
    let mut vues: usize = 0;
    loop {
        if max.is_some_and(|m| vues >= m) {
            return Ok(());
        }
        if limite.is_some_and(|l| indice >= l) {
            return Ok(());
        }
        let Some(candidat) = occurrence(regle, debut, indice) else {
            return Ok(());
        };
        if candidat > jusqu_a || fin.is_some_and(|f| candidat > f) {
            return Ok(());
        }
        vues = vues.saturating_add(1);
        if !visite(candidat) {
            return Ok(());
        }
        indice = indice.saturating_add(1);
    }
}

/// La `indice`-ieme echeance de la serie, la zero etant le debut.
///
/// Rend `None` quand le calcul sort du calendrier representable.
fn occurrence(regle: &Regle, debut: Date, indice: u64) -> Option<Date> {
    if indice == 0 {
        return Some(debut);
    }
    let (annee_debut, mois_debut, _) = debut.pieces();
    let pas = i64::from(regle.frequence.pas_en_mois());
    let decalage = i64::try_from(indice).ok()?.checked_mul(pas)?;
    let total = annee_debut
        .checked_mul(12)?
        .checked_add(i64::from(mois_debut) - 1)?
        .checked_add(decalage)?;
    let annee = total.div_euclid(12);
    if !(1..=9999).contains(&annee) {
        return None;
    }
    let mois = total.rem_euclid(12) as u32 + 1;
    let jour = regle.jour.min(jours_dans_mois(annee, mois));
    Some(Date {
        jours: jours_depuis_civil(annee, mois, jour),
    })
}

/// Point de depart du parcours vers l'avant : l'indice le plus grand dont on
/// est sur qu'il ne depasse pas la reponse cherchee.
///
/// Sans cette estimation, chercher « la prochaine apres dans dix ans » ferait
/// cent vingt tours de boucle.
fn indice_de_depart(debut: Date, apres: Date, pas: u32) -> u64 {
    let ecart = apres.mois_absolu() - debut.mois_absolu();
    if ecart <= 0 {
        return 0;
    }
    (ecart / i64::from(pas)) as u64
}

/// Une date civile, portee comme un nombre de jours depuis le 1er janvier 1970.
///
/// L'ordre des entiers est l'ordre chronologique : comparer deux dates revient
/// a comparer leurs jours, sans cas particulier de fin de mois.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Date {
    jours: i64,
}

/// Decalage de l'epoque Unix dans le calendrier de Hinnant.
const EPOQUE: i64 = 719_468;

impl Date {
    /// Lit `AAAA-MM-JJ` et verifie que la date existe reellement.
    ///
    /// Le format est exige tel quel : `2026-1-1` et `01/02/2026` sont refuses.
    /// Un `2026-02-30` aussi : une date qui n'existe pas ne doit pas entrer
    /// dans une facture.
    fn analyser(texte: &str) -> Result<Self, Erreur> {
        let octets = texte.as_bytes();
        let forme = octets.len() == 10
            && octets[4] == b'-'
            && octets[7] == b'-'
            && octets[..4].iter().all(u8::is_ascii_digit)
            && octets[5..7].iter().all(u8::is_ascii_digit)
            && octets[8..].iter().all(u8::is_ascii_digit);
        if !forme {
            return Err(Erreur::DateInvalide(texte.to_string()));
        }
        let annee = nombre(&octets[..4]);
        let mois = nombre(&octets[5..7]);
        let jour = nombre(&octets[8..]);
        if !(1..=9999).contains(&annee) || !(1..=12).contains(&mois) || !(1..=31).contains(&jour) {
            return Err(Erreur::DateInvalide(texte.to_string()));
        }
        if jour > jours_dans_mois(i64::from(annee), mois) {
            return Err(Erreur::DateInvalide(texte.to_string()));
        }
        Ok(Date {
            jours: jours_depuis_civil(i64::from(annee), mois, jour),
        })
    }

    /// Annee, mois, jour.
    fn pieces(self) -> (i64, u32, u32) {
        civil_depuis_jours(self.jours)
    }

    /// Le mois compte depuis l'an 1, pour mesurer un ecart en mois sans se
    /// soucier des changements d'annee.
    fn mois_absolu(self) -> i64 {
        let (annee, mois, _) = self.pieces();
        annee * 12 + i64::from(mois) - 1
    }

    /// Rend la date en `AAAA-MM-JJ`, zero de remplissage compris.
    fn texte(self) -> String {
        let (annee, mois, jour) = self.pieces();
        format!("{annee:04}-{mois:02}-{jour:02}")
    }
}

/// Lit un nombre decimal sur des octets deja verifies chiffres.
fn nombre(octets: &[u8]) -> u32 {
    octets
        .iter()
        .fold(0u32, |valeur, &octet| valeur * 10 + u32::from(octet - b'0'))
}

/// Nombre de jours du mois, par difference avec le premier jour du mois
/// suivant : aucune table, donc aucun oubli d'annee bissextile.
fn jours_dans_mois(annee: i64, mois: u32) -> u32 {
    let (annee_suivante, mois_suivant) = if mois == 12 {
        (annee + 1, 1)
    } else {
        (annee, mois + 1)
    };
    let debut = jours_depuis_civil(annee, mois, 1);
    let suite = jours_depuis_civil(annee_suivante, mois_suivant, 1);
    (suite - debut) as u32
}

/// Jours ecoules depuis le 1er janvier 1970 (`days_from_civil` de Hinnant).
///
/// L'annee est coupee en eres de 400 ans, la seule duree dont la longueur en
/// jours est constante : 146 097. Le mois est ramene a un compte partant de
/// mars pour que le 29 fevrier tombe en fin d'annee et ne perturbe pas les
/// divisions.
fn jours_depuis_civil(annee: i64, mois: u32, jour: u32) -> i64 {
    let annee = if mois <= 2 { annee - 1 } else { annee };
    let ere = if annee >= 0 { annee } else { annee - 399 } / 400;
    let annee_ere = annee - ere * 400;
    let mois_mars = (i64::from(mois) + 9) % 12;
    let jour_annee = (153 * mois_mars + 2) / 5 + i64::from(jour) - 1;
    let jour_ere = annee_ere * 365 + annee_ere / 4 - annee_ere / 100 + jour_annee;
    ere * 146_097 + jour_ere - EPOQUE
}

/// Inverse de [`jours_depuis_civil`] (`civil_from_days` de Hinnant).
fn civil_depuis_jours(jours: i64) -> (i64, u32, u32) {
    let z = jours + EPOQUE;
    let ere = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let jour_ere = z - ere * 146_097;
    let annee_ere = (jour_ere - jour_ere / 1460 + jour_ere / 36_524 - jour_ere / 146_096) / 365;
    let annee = annee_ere + ere * 400;
    let jour_annee = jour_ere - (365 * annee_ere + annee_ere / 4 - annee_ere / 100);
    let mois_mars = (5 * jour_annee + 2) / 153;
    let jour = (jour_annee - (153 * mois_mars + 2) / 5 + 1) as u32;
    let mois = (mois_mars + if mois_mars < 10 { 3 } else { -9 }) as u32;
    (annee + i64::from(mois <= 2), mois, jour)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une regle mensuelle sans fin, pour isoler une seule chose a la fois.
    fn mensuelle(debut: &str, jour: u32) -> Regle {
        Regle {
            frequence: Frequence::Mensuelle,
            debut: debut.to_string(),
            fin: None,
            limite_occurrences: None,
            jour,
        }
    }

    /// La suivante ou la fin de la serie.
    fn suivante(regle: &Regle, apres: &str) -> Option<String> {
        prochaine(regle, apres).expect("regle et date valides")
    }

    /// Les echeances ou l'erreur, en texte lisible.
    fn serie(regle: &Regle, jusqu_a: &str, max: usize) -> Vec<String> {
        echeances(regle, jusqu_a, max).expect("regle et date valides")
    }

    #[test]
    fn fin_de_mois_au_31_tombe_sur_le_dernier_jour_court() {
        let regle = mensuelle("2026-01-31", 31);
        // Fevrier 2026 a 28 jours : on veut le 28, pas le 3 mars.
        assert_eq!(
            suivante(&regle, "2026-01-31"),
            Some("2026-02-28".to_string())
        );
        // Mars reprend le 31 : la regle ne derive pas d'un jour par mois.
        assert_eq!(
            suivante(&regle, "2026-02-28"),
            Some("2026-03-31".to_string())
        );
        // Avril a 30 jours, mai 31, juin 30.
        assert_eq!(
            suivante(&regle, "2026-03-31"),
            Some("2026-04-30".to_string())
        );
        assert_eq!(
            suivante(&regle, "2026-04-30"),
            Some("2026-05-31".to_string())
        );
        assert_eq!(
            suivante(&regle, "2026-05-31"),
            Some("2026-06-30".to_string())
        );
    }

    #[test]
    fn fin_de_mois_au_30_tombe_sur_le_dernier_jour_court() {
        let regle = mensuelle("2026-01-30", 30);
        assert_eq!(
            suivante(&regle, "2026-01-30"),
            Some("2026-02-28".to_string())
        );
        assert_eq!(
            suivante(&regle, "2026-02-28"),
            Some("2026-03-30".to_string())
        );
        // Avril a 30 jours : le 30 existe, rien a ramener.
        assert_eq!(
            suivante(&regle, "2026-03-30"),
            Some("2026-04-30".to_string())
        );
    }

    #[test]
    fn annee_bissextile_le_29_se_rabat_sur_le_28() {
        let regle = mensuelle("2024-02-29", 29);
        assert_eq!(
            suivante(&regle, "2024-02-29"),
            Some("2024-03-29".to_string())
        );
        // 2025 n'est pas bissextile : le 29 fevrier n'existe pas, on rend le 28.
        let annuelle = Regle {
            frequence: Frequence::Annuelle,
            debut: "2024-02-29".to_string(),
            fin: None,
            limite_occurrences: None,
            jour: 29,
        };
        assert_eq!(
            suivante(&annuelle, "2024-02-29"),
            Some("2025-02-28".to_string())
        );
        // 2028 est bissextile : le 29 revient, la regle ne s'est pas abimee.
        assert_eq!(
            suivante(&annuelle, "2027-02-28"),
            Some("2028-02-29".to_string())
        );
    }

    #[test]
    fn la_fin_arrete_la_serie() {
        let regle = Regle {
            fin: Some("2026-03-31".to_string()),
            ..mensuelle("2026-01-31", 31)
        };
        assert_eq!(
            suivante(&regle, "2026-01-31"),
            Some("2026-02-28".to_string())
        );
        assert_eq!(
            suivante(&regle, "2026-02-28"),
            Some("2026-03-31".to_string())
        );
        // La fin est incluse : au-dela, la serie est close.
        assert_eq!(suivante(&regle, "2026-03-31"), None);
        assert_eq!(suivante(&regle, "2027-01-01"), None);
        assert_eq!(
            serie(&regle, "2026-12-31", 100),
            ["2026-01-31", "2026-02-28", "2026-03-31"]
        );
    }

    #[test]
    fn la_limite_d_occurrences_compte_le_debut() {
        let regle = Regle {
            limite_occurrences: Some(3),
            ..mensuelle("2026-01-31", 31)
        };
        // Trois echeances : janvier, fevrier, mars. Le debut compte pour une.
        assert_eq!(
            suivante(&regle, "2025-12-31"),
            Some("2026-01-31".to_string())
        );
        assert_eq!(
            suivante(&regle, "2026-01-31"),
            Some("2026-02-28".to_string())
        );
        assert_eq!(
            suivante(&regle, "2026-02-28"),
            Some("2026-03-31".to_string())
        );
        assert_eq!(suivante(&regle, "2026-03-31"), None);
        assert_eq!(
            serie(&regle, "2030-12-31", 100),
            ["2026-01-31", "2026-02-28", "2026-03-31"]
        );
    }

    #[test]
    fn une_limite_nulle_est_refusee() {
        let regle = Regle {
            limite_occurrences: Some(0),
            ..mensuelle("2026-01-31", 31)
        };
        assert!(matches!(
            prochaine(&regle, "2026-01-31"),
            Err(Erreur::RegleInvalide(_))
        ));
    }

    #[test]
    fn echeances_ordonnees_sans_doublon_ni_depassement() {
        let regle = mensuelle("2026-01-31", 31);
        let dates = serie(&regle, "2026-06-30", 100);
        assert_eq!(
            dates,
            [
                "2026-01-31",
                "2026-02-28",
                "2026-03-31",
                "2026-04-30",
                "2026-05-31",
                "2026-06-30"
            ]
        );
        // Ordre croissant strict : ni doublon, ni retour en arriere.
        assert!(dates.windows(2).all(|paire| paire[0] < paire[1]));
        // Jamais au-dela de la borne demandee.
        assert!(dates.iter().all(|date| date.as_str() <= "2026-06-30"));
    }

    #[test]
    fn echeances_respecte_le_maximum() {
        let regle = mensuelle("2026-01-31", 31);
        assert_eq!(
            serie(&regle, "2026-12-31", 3),
            ["2026-01-31", "2026-02-28", "2026-03-31"]
        );
        // Un maximum nul rend une liste vide, sans calcul.
        assert!(serie(&regle, "2026-12-31", 0).is_empty());
        // Borne anterieure au debut : rien a emettre.
        assert!(serie(&regle, "2025-12-31", 10).is_empty());
    }

    #[test]
    fn prochaine_avant_le_debut_rend_la_premiere_echeance() {
        let regle = mensuelle("2026-01-31", 31);
        assert_eq!(
            suivante(&regle, "2026-01-01"),
            Some("2026-01-31".to_string())
        );
        // Loin devant : on saute directement, la reponse reste la meme.
        assert_eq!(
            suivante(&regle, "2036-05-15"),
            Some("2036-05-31".to_string())
        );
    }

    #[test]
    fn le_jour_voulu_ne_change_que_les_echeances_suivantes() {
        // Un contrat signe le 15 mais prelevable en fin de mois : la premiere
        // echeance reste le 15, la date du contrat, et les suivantes tombent
        // au dernier jour du mois.
        let regle = mensuelle("2026-01-15", 31);
        assert_eq!(
            serie(&regle, "2026-04-30", 10),
            ["2026-01-15", "2026-02-28", "2026-03-31", "2026-04-30"]
        );
    }

    #[test]
    fn nombre_restant_s_arrete_a_la_regle() {
        let regle = mensuelle("2026-01-31", 31);
        assert_eq!(nombre_restant(&regle, "2026-12-31").expect("valide"), 12);
        assert_eq!(nombre_restant(&regle, "2026-01-31").expect("valide"), 1);
        assert_eq!(nombre_restant(&regle, "2025-12-31").expect("valide"), 0);
        // La limite de la regle tronque le total.
        let bornee = Regle {
            limite_occurrences: Some(5),
            ..mensuelle("2026-01-31", 31)
        };
        assert_eq!(nombre_restant(&bornee, "2030-12-31").expect("valide"), 5);
        // La fin aussi.
        let close = Regle {
            fin: Some("2026-03-31".to_string()),
            ..mensuelle("2026-01-31", 31)
        };
        assert_eq!(nombre_restant(&close, "2030-12-31").expect("valide"), 3);
    }

    #[test]
    fn refus_des_dates_mal_formees() {
        let regle = mensuelle("2026-01-31", 31);
        assert_eq!(
            prochaine(&regle, "2026-13-01"),
            Err(Erreur::DateInvalide("2026-13-01".to_string()))
        );
        assert_eq!(
            prochaine(&regle, "01/02/2026"),
            Err(Erreur::DateInvalide("01/02/2026".to_string()))
        );
        // Zero de remplissage absent.
        assert_eq!(
            prochaine(&regle, "2026-1-1"),
            Err(Erreur::DateInvalide("2026-1-1".to_string()))
        );
        // Date qui n'existe pas au calendrier.
        assert_eq!(
            prochaine(&regle, "2026-02-30"),
            Err(Erreur::DateInvalide("2026-02-30".to_string()))
        );
        // La borne haute de echeances est verifiee comme les autres.
        assert_eq!(
            echeances(&regle, "2026-00-10", 5),
            Err(Erreur::DateInvalide("2026-00-10".to_string()))
        );
        // Le debut de la regle aussi.
        let cassee = mensuelle("2026-02-31", 31);
        assert!(matches!(
            prochaine(&cassee, "2026-01-01"),
            Err(Erreur::DateInvalide(_))
        ));
    }

    #[test]
    fn refus_du_jour_hors_bornes() {
        assert_eq!(
            prochaine(&mensuelle("2026-01-15", 0), "2026-01-15"),
            Err(Erreur::JourInvalide(0))
        );
        assert_eq!(
            prochaine(&mensuelle("2026-01-15", 32), "2026-01-15"),
            Err(Erreur::JourInvalide(32))
        );
        assert_eq!(
            echeances(&mensuelle("2026-01-15", 99), "2026-12-31", 5),
            Err(Erreur::JourInvalide(99))
        );
    }

    #[test]
    fn refus_d_une_fin_anterieure_au_debut() {
        let regle = Regle {
            fin: Some("2025-12-31".to_string()),
            ..mensuelle("2026-01-31", 31)
        };
        assert!(matches!(
            prochaine(&regle, "2026-01-31"),
            Err(Erreur::RegleInvalide(_))
        ));
        assert!(matches!(
            nombre_restant(&regle, "2026-12-31"),
            Err(Erreur::RegleInvalide(_))
        ));
        // Une fin egale au debut reste valable : une seule echeance.
        let unique = Regle {
            fin: Some("2026-01-31".to_string()),
            ..mensuelle("2026-01-31", 31)
        };
        assert_eq!(serie(&unique, "2026-12-31", 10), ["2026-01-31"]);
    }

    #[test]
    fn les_erreurs_s_expliquent_en_francais() {
        assert_eq!(
            Erreur::DateInvalide("2026-13-01".to_string()).to_string(),
            "date invalide : 2026-13-01"
        );
        assert_eq!(
            Erreur::RegleInvalide("fin anterieure".to_string()).to_string(),
            "regle invalide : fin anterieure"
        );
        assert_eq!(Erreur::JourInvalide(0).to_string(), "jour invalide : 0");
    }

    #[test]
    fn les_frequences_donnent_leur_pas_en_mois() {
        assert_eq!(Frequence::Mensuelle.pas_en_mois(), 1);
        assert_eq!(Frequence::Trimestrielle.pas_en_mois(), 3);
        assert_eq!(Frequence::Semestrielle.pas_en_mois(), 6);
        assert_eq!(Frequence::Annuelle.pas_en_mois(), 12);
        assert_eq!(Frequence::Trimestrielle.libelle(), "Trimestrielle");
    }

    #[test]
    fn les_frequences_longues_ne_glissent_pas_de_mois() {
        // Trimestrielle au 31 janvier : avril, juillet, octobre, pas mars.
        let regle = Regle {
            frequence: Frequence::Trimestrielle,
            ..mensuelle("2026-01-31", 31)
        };
        assert_eq!(
            serie(&regle, "2026-10-31", 10),
            ["2026-01-31", "2026-04-30", "2026-07-31", "2026-10-31"]
        );
        let semestrielle = Regle {
            frequence: Frequence::Semestrielle,
            ..mensuelle("2026-01-31", 31)
        };
        assert_eq!(
            serie(&semestrielle, "2027-01-31", 10),
            ["2026-01-31", "2026-07-31", "2027-01-31"]
        );
    }

    #[test]
    fn le_calendrier_fait_l_aller_et_retour() {
        // Quelques dates de reference, dont deux fins de mois bissextiles.
        for (annee, mois, jour) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (2024, 2, 29),
            (2026, 12, 31),
            (9999, 12, 31),
        ] {
            let jours = jours_depuis_civil(annee, mois, jour);
            assert_eq!(civil_depuis_jours(jours), (annee, mois, jour));
        }
        // 1er mars 2026 suit le 28 fevrier : un jour d'ecart, pas trois.
        assert_eq!(
            jours_depuis_civil(2026, 3, 1) - jours_depuis_civil(2026, 2, 28),
            1
        );
        assert_eq!(jours_dans_mois(2024, 2), 29);
        assert_eq!(jours_dans_mois(2025, 2), 28);
        assert_eq!(jours_dans_mois(2026, 4), 30);
    }
}
