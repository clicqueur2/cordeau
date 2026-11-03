//! Moteur de traduction du noyau : un [`Catalogue`] de textes par langue.
//!
//! Aujourd'hui l'interface est ecrite en francais dans le code. Ce module existe
//! pour que ce ne soit plus une fatalite : les textes vivent dans un catalogue,
//! on les charge depuis des fichiers au format Dolibarr `.lang`, et l'ecran
//! demande une cle — jamais une phrase en dur.
//!
//! # La cascade de repli
//!
//! [`Catalogue::t`] cherche dans cet ordre :
//!
//! 1. la langue demandee ;
//! 2. la langue par defaut du catalogue ;
//! 3. la cle elle-meme.
//!
//! Elle ne rend **jamais une chaine vide**. Une traduction absente se voit (on
//! lit la cle, `TotalHT`), elle ne laisse pas un trou blanc a l'ecran. Une
//! traduction presente mais vide (`Cle=` dans le fichier) est traitee comme
//! absente : c'est une ligne oubliee, pas une volonte de ne rien afficher.
//!
//! # Le format `.lang` de Dolibarr
//!
//! Une ligne par traduction, `Cle=valeur`. Les lignes vides et les commentaires
//! (`#`) sont ignores, la valeur garde les `=` qu'elle contient, et les
//! sequences `\n`, `\t`, `\r` sont converties en vrais caracteres — c'est ainsi
//! qu'un libelle tient sur deux lignes dans un fichier langue.
//!
//! # Les codes de langue
//!
//! Un code est normalise des l'entree : minuscules, et seule la partie
//! principale est retenue. `fr-FR`, `fr_FR` et `fr` designent donc le meme
//! francais, et l'appelant n'a pas a savoir sous quelle forme la traduction a
//! ete enregistree.
//!
//! ```
//! use i18n::Catalogue;
//!
//! let mut c = Catalogue::nouveau("fr");
//! c.depuis_texte("fr", "Bonjour=Bonjour {nom}\n");
//! c.ajouter("en", "Bonjour", "Hello {nom}");
//!
//! assert_eq!(c.t_args("en", "Bonjour", &[("nom", "Maxime")]), "Hello Maxime");
//! // Cle absente en anglais : on retombe sur le francais.
//! c.depuis_texte("fr", "Merci=Merci beaucoup\n");
//! assert_eq!(c.t("en", "Merci"), "Merci beaucoup");
//! ```

use std::collections::BTreeMap;

/// Langue retenue quand rien d'exploitable n'est fourni : ni en-tete
/// `Accept-Language` lisible, ni langue par defaut valide.
pub const LANGUE_PAR_DEFAUT: &str = "fr";

/// Ce qui empeche d'enregistrer une traduction.
#[derive(Debug, PartialEq, Eq)]
pub enum Erreur {
    /// La cle est vide (ou faite d'espaces) : elle ne pourrait jamais etre
    /// demandee, donc la ligne ne sert a rien.
    CleVide,
    /// Le code de langue est vide ou ne ressemble pas a une langue
    /// (`fr`, `fr-FR`, `fr_FR`) : on refuse plutot que de creer une langue
    /// fantome qui n'apparaitrait dans aucune interface.
    LangueVide,
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::CleVide => write!(f, "cle de traduction vide"),
            Erreur::LangueVide => write!(f, "code de langue vide ou invalide"),
        }
    }
}

impl std::error::Error for Erreur {}

/// Retire un eventuel parametre (`fr;q=0.9` -> `fr`).
fn sans_parametre(brut: &str) -> &str {
    match brut.split_once(';') {
        Some((tete, _)) => tete,
        None => brut,
    }
}

/// Normalise un code de langue : minuscules, partie principale seulement.
///
/// Rend `None` si le code ne peut designer aucune langue : vide, `*`, ou
/// quelque chose qui n'est pas fait de lettres (`###`, `?`).
fn normaliser_langue(brut: &str) -> Option<String> {
    let tete = sans_parametre(brut).trim();
    if tete.is_empty() || tete == "*" {
        return None;
    }
    let coupe = match tete
        .char_indices()
        .find(|(_, c)| *c == '-' || *c == '_')
        .map(|(i, _)| i)
    {
        Some(i) => &tete[..i],
        None => tete,
    };
    let coupe = coupe.trim().to_ascii_lowercase();
    // Une sous-etiquette principale fait de 2 a 8 lettres (RFC 5646). Ce
    // garde-fou ecarte les en-tetes abimes au lieu d'inventer une langue.
    if !(2..=8).contains(&coupe.len()) || !coupe.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(coupe)
}

/// Convertit les sequences d'echappement d'un fichier `.lang` en vrais
/// caracteres. `\n` devient un retour a la ligne ; une sequence inconnue est
/// laissee telle quelle pour ne rien perdre en silence.
fn desetoffer(texte: &str) -> String {
    let mut sortie = String::with_capacity(texte.len());
    let mut iter = texte.chars();
    while let Some(caractere) = iter.next() {
        if caractere != '\\' {
            sortie.push(caractere);
            continue;
        }
        match iter.next() {
            Some('n') => sortie.push('\n'),
            Some('r') => sortie.push('\r'),
            Some('t') => sortie.push('\t'),
            Some('\\') => sortie.push('\\'),
            Some(autre) => {
                sortie.push('\\');
                sortie.push(autre);
            }
            None => sortie.push('\\'),
        }
    }
    sortie
}

/// Lit la valeur de qualite d'un parametre `Accept-Language`, en millièmes
/// (1000 = prioritaire, 0 = refuse). Rend `None` si ce n'est pas un `q=`.
fn lire_qualite(parametre: &str) -> Option<u16> {
    let (nom, valeur) = parametre.split_once('=')?;
    if !nom.trim().eq_ignore_ascii_case("q") {
        return None;
    }
    let brut: f64 = valeur.trim().parse().ok()?;
    // Un `q` hors bornes ou abime ne doit pas faire perdre la langue : on
    // borne, et une valeur illisible laisse la priorite par defaut.
    let borne = brut.clamp(0.0, 1.0);
    Some((borne * 1000.0).round() as u16)
}

/// Depouille un en-tete `Accept-Language` en langues candidates, la plus
/// probable d'abord.
///
/// L'ordre de l'en-tete tranche les egalites ; les langues declarees `q=0`
/// sont ecartees, `*` est ignore (il ne dit rien de la langue voulue).
fn candidats(entete: &str) -> Vec<(String, u16)> {
    let mut liste: Vec<(String, u16)> = entete
        .split(',')
        .filter_map(|element| {
            let mut morceaux = element.split(';');
            let tete = morceaux.next()?;
            let langue = normaliser_langue(tete)?;
            let mut qualite = 1000;
            for parametre in morceaux {
                if let Some(q) = lire_qualite(parametre) {
                    qualite = q;
                }
            }
            if qualite == 0 {
                return None;
            }
            Some((langue, qualite))
        })
        .collect();
    // Tri stable : a qualite egale, l'ordre de l'en-tete est conserve.
    liste.sort_by_key(|(_, qualite)| std::cmp::Reverse(*qualite));
    liste
}

/// Devine la langue d'un visiteur a partir de son en-tete `Accept-Language`.
///
/// `fr-FR,fr;q=0.9,en;q=0.8` donne `fr`, `en-US,en;q=0.9` donne `en`. Un en-tete
/// vide ou incomprehensible rend [`LANGUE_PAR_DEFAUT`]. La langue rendue est
/// toujours normalisee (partie principale, minuscules).
///
/// Cette fonction ne connait pas le catalogue : pour choisir parmi les langues
/// reellement disponibles, voir [`Catalogue::devine_langue`].
pub fn devine_langue(entete: &str) -> String {
    match candidats(entete).into_iter().next() {
        Some((langue, _)) => langue,
        None => LANGUE_PAR_DEFAUT.to_string(),
    }
}

/// Les traductions, rangees par langue puis par cle.
///
/// Les langues sont gardees en [`BTreeMap`] : [`Catalogue::langues`] est donc
/// deja triee, et l'ordre ne depend pas de l'ordre des insertions.
#[derive(Debug, Clone)]
pub struct Catalogue {
    /// Langue de repli, deja normalisee, jamais vide.
    langue_par_defaut: String,
    /// langue -> (cle -> texte). Une langue n'apparait ici qu'a partir du
    /// moment ou elle porte au moins une traduction.
    traductions: BTreeMap<String, BTreeMap<String, String>>,
}

impl Catalogue {
    /// Cree un catalogue vide dont la langue de repli est `langue_par_defaut`.
    ///
    /// Un code vide, `*` ou illisible retombe sur [`LANGUE_PAR_DEFAUT`] : un
    /// catalogue sans langue de repli n'aurait aucun sens.
    pub fn nouveau(langue_par_defaut: &str) -> Self {
        Catalogue {
            langue_par_defaut: normaliser_langue(langue_par_defaut)
                .unwrap_or_else(|| LANGUE_PAR_DEFAUT.to_string()),
            traductions: BTreeMap::new(),
        }
    }

    /// La langue de repli, normalisee.
    pub fn langue_par_defaut(&self) -> &str {
        &self.langue_par_defaut
    }

    /// Enregistre une traduction, en ignorant silencieusement une entree
    /// invalide (langue ou cle vide).
    ///
    /// C'est la porte d'entree des chargements en masse ou l'on ne veut pas
    /// s'arreter sur une ligne fautive ; [`Catalogue::ajouter_verifie`] dit, elle,
    /// pourquoi elle refuse.
    pub fn ajouter(&mut self, langue: &str, cle: &str, texte: &str) {
        // Une entree sans langue ni cle ne serait jamais relue : la jeter est
        // le seul comportement qui ne casse rien.
        let _ = self.ajouter_verifie(langue, cle, texte);
    }

    /// Enregistre une traduction en signalant ce qui l'empeche.
    ///
    /// Une cle deja presente est remplacee : recharger un fichier corrige le
    /// catalogue au lieu d'y empiler des doublons.
    pub fn ajouter_verifie(&mut self, langue: &str, cle: &str, texte: &str) -> Result<(), Erreur> {
        let langue = normaliser_langue(langue).ok_or(Erreur::LangueVide)?;
        let cle = cle.trim();
        if cle.is_empty() {
            return Err(Erreur::CleVide);
        }
        self.traductions
            .entry(langue)
            .or_default()
            .insert(cle.to_string(), texte.to_string());
        Ok(())
    }

    /// Charge un fichier au format Dolibarr `.lang` pour une langue, et rend le
    /// nombre de cles enregistrees.
    ///
    /// Les lignes vides, les commentaires (`#`) et les lignes sans `=` sont
    /// ignores ; la valeur garde ses `=` et ses espaces internes, les `\n`
    /// litteraux deviennent de vrais retours a la ligne. Un eventuel BOM en
    /// tete de fichier est retire.
    pub fn depuis_texte(&mut self, langue: &str, contenu: &str) -> usize {
        let contenu = contenu.strip_prefix('\u{feff}').unwrap_or(contenu);
        let mut nb = 0;
        for ligne in contenu.lines() {
            let ligne = ligne.trim();
            if ligne.is_empty() || ligne.starts_with('#') {
                continue;
            }
            let Some((cle, valeur)) = ligne.split_once('=') else {
                continue;
            };
            let valeur = desetoffer(valeur.trim());
            if self.ajouter_verifie(langue, cle, &valeur).is_ok() {
                nb += 1;
            }
        }
        nb
    }

    /// La traduction de `cle`, ou son repli : langue demandee, puis langue par
    /// defaut, puis la cle elle-meme. Jamais de chaine vide pour une cle non
    /// vide.
    pub fn t(&self, langue: &str, cle: &str) -> String {
        match self.cherche(langue, cle) {
            Some(texte) => texte.to_string(),
            None => cle.to_string(),
        }
    }

    /// Comme [`Catalogue::t`], puis remplace chaque `{nom}` par la valeur
    /// fournie.
    ///
    /// Un argument sans `{...}` correspondant est simplement ignore ; un
    /// `{nom}` sans argument reste affiche tel quel, pour qu'on voie le
    /// manque plutot que de lire une phrase trouee.
    pub fn t_args(&self, langue: &str, cle: &str, args: &[(&str, &str)]) -> String {
        let modele = self.t(langue, cle);
        if args.is_empty() || !modele.contains('{') {
            return modele;
        }
        let table: BTreeMap<&str, &str> = args.iter().copied().collect();
        let mut sortie = String::with_capacity(modele.len());
        let mut reste = modele.as_str();
        while let Some(debut) = reste.find('{') {
            sortie.push_str(&reste[..debut]);
            let apres = &reste[debut + 1..];
            match apres.find('}') {
                Some(fin) => {
                    let nom = &apres[..fin];
                    match table.get(nom) {
                        // La valeur est recopiee telle quelle, sans etre
                        // reinterpretee : elle ne peut pas injecter un autre
                        // `{...}`.
                        Some(valeur) => sortie.push_str(valeur),
                        None => {
                            sortie.push('{');
                            sortie.push_str(nom);
                            sortie.push('}');
                        }
                    }
                    reste = &apres[fin + 1..];
                }
                None => {
                    // Accolade non fermee : on recopie la fin sans y toucher.
                    sortie.push_str(&reste[debut..]);
                    reste = "";
                }
            }
        }
        sortie.push_str(reste);
        sortie
    }

    /// La recherche brute, sans repli sur la cle : `None` veut dire « rien
    /// d'affichable ».
    fn cherche(&self, langue: &str, cle: &str) -> Option<&str> {
        let cle = cle.trim();
        if cle.is_empty() {
            return None;
        }
        // 1. la langue demandee.
        if let Some(texte) = normaliser_langue(langue)
            .and_then(|langue| self.traductions.get(&langue))
            .and_then(|textes| textes.get(cle))
            .filter(|texte| !texte.is_empty())
        {
            return Some(texte);
        }
        // 2. la langue par defaut du catalogue.
        self.traductions
            .get(&self.langue_par_defaut)
            .and_then(|textes| textes.get(cle))
            .filter(|texte| !texte.is_empty())
            .map(|texte| texte.as_str())
    }

    /// Les langues qui portent au moins une traduction, triees.
    pub fn langues(&self) -> Vec<&str> {
        self.traductions
            .iter()
            .filter(|(_, textes)| !textes.is_empty())
            .map(|(langue, _)| langue.as_str())
            .collect()
    }

    /// Le nombre de cles connues pour une langue (0 si elle est inconnue).
    pub fn nb_cles(&self, langue: &str) -> usize {
        normaliser_langue(langue)
            .and_then(|langue| self.traductions.get(&langue))
            .map_or(0, |textes| textes.len())
    }

    /// La langue la plus probable de l'en-tete, **parmi celles du catalogue**,
    /// avec repli sur la langue par defaut du catalogue.
    ///
    /// C'est la variante a utiliser cote serveur : un visiteur qui demande de
    /// l'allemand sur un site qui n'a que le francais et l'anglais doit obtenir
    /// du francais, pas une interface allemande vide.
    pub fn devine_langue(&self, entete: &str) -> String {
        for (langue, _) in candidats(entete) {
            if self.traductions.contains_key(&langue) {
                return langue;
            }
        }
        self.langue_par_defaut.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Catalogue {
        let mut c = Catalogue::nouveau("fr");
        c.ajouter("fr", "Bonjour", "Bonjour");
        c.ajouter("fr", "BonjourNom", "Bonjour {nom}");
        c.ajouter("en", "AuRevoir", "Goodbye");
        c
    }

    #[test]
    fn repli_en_cascade_jusqua_la_cle() {
        let c = catalogue();
        // La cle existe en anglais : on la sert en anglais.
        assert_eq!(c.t("en", "AuRevoir"), "Goodbye");
        // Absente en anglais, presente en francais : repli sur le francais.
        assert_eq!(c.t("en", "Bonjour"), "Bonjour");
        // Absente partout : on affiche la cle, jamais du blanc.
        assert_eq!(c.t("en", "TotalHT"), "TotalHT");
        assert_eq!(c.t("de", "TotalHT"), "TotalHT");
        assert!(!c.t("de", "TotalHT").is_empty());
        // Langue vide ou inconnue : on se rabat sur la langue par defaut.
        assert_eq!(c.t("", "Bonjour"), "Bonjour");
        assert_eq!(c.t("zz", "Bonjour"), "Bonjour");
        // Une traduction presente mais vide compte comme absente.
        let mut vide = Catalogue::nouveau("fr");
        vide.ajouter("fr", "Cle", "");
        assert_eq!(vide.t("fr", "Cle"), "Cle");
    }

    #[test]
    fn format_lang_dolibarr_commentaires_et_sauts_de_ligne() {
        let fichier = concat!(
            "\u{feff}# Fichier de langue francais\n",
            "\n",
            "   # commentaire indente\n",
            "Accueil=Accueil\n",
            "DeuxLignes=Ligne 1\\nLigne 2\n",
            "Tabulation=A\\tB\n",
            "Formule=Devis = facture\n",
            "SansEgal\n",
            "Vide=\n",
            "Espace = Valeur avec espaces \n",
        );
        let mut c = Catalogue::nouveau("fr");
        let nb = c.depuis_texte("fr", fichier);
        // Accueil, DeuxLignes, Tabulation, Formule, Vide, Espace.
        assert_eq!(nb, 6);
        assert_eq!(c.nb_cles("fr"), 6);

        assert_eq!(c.t("fr", "Accueil"), "Accueil");
        // Le \n litteral devient un vrai retour a la ligne.
        assert_eq!(c.t("fr", "DeuxLignes"), "Ligne 1\nLigne 2");
        assert_eq!(c.t("fr", "Tabulation"), "A\tB");
        // La valeur garde les = qu'elle contient.
        assert_eq!(c.t("fr", "Formule"), "Devis = facture");
        assert_eq!(c.t("fr", "Espace"), "Valeur avec espaces");
        // La ligne sans = est ignoree : la cle reste inconnue.
        assert_eq!(c.t("fr", "SansEgal"), "SansEgal");

        // Un rechargement remplace au lieu d'empiler.
        assert_eq!(c.depuis_texte("fr", "Accueil=Bienvenue\n"), 1);
        assert_eq!(c.t("fr", "Accueil"), "Bienvenue");
        assert_eq!(c.nb_cles("fr"), 6);
    }

    #[test]
    fn substitution_des_arguments() {
        let c = catalogue();
        assert_eq!(
            c.t_args("fr", "BonjourNom", &[("nom", "Maxime")]),
            "Bonjour Maxime"
        );
        // Un argument sans placeholder correspondant ne casse rien.
        assert_eq!(
            c.t_args("fr", "BonjourNom", &[("nom", "Maxime"), ("ville", "Lyon")]),
            "Bonjour Maxime"
        );
        // Un placeholder sans argument reste visible.
        assert_eq!(c.t_args("fr", "BonjourNom", &[]), "Bonjour {nom}");
        assert_eq!(
            c.t_args("fr", "BonjourNom", &[("autre", "x")]),
            "Bonjour {nom}"
        );
        // Placeholder repete, et valeur contenant des accolades non
        // reinterpretee.
        let mut c = catalogue();
        c.ajouter("fr", "Deux", "{a} et {b} et {a}");
        assert_eq!(
            c.t_args("fr", "Deux", &[("a", "1"), ("b", "2")]),
            "1 et 2 et 1"
        );
        c.ajouter("fr", "Piege", "{a}");
        assert_eq!(c.t_args("fr", "Piege", &[("a", "{b}"), ("b", "X")]), "{b}");
        // Accolade non fermee : le texte est rendu tel quel.
        c.ajouter("fr", "Ouverte", "Total { sans fin");
        assert_eq!(c.t_args("fr", "Ouverte", &[("x", "y")]), "Total { sans fin");
        // Cle inconnue : la cle fait office de modele, sans casser.
        assert_eq!(c.t_args("fr", "Inconnue", &[("nom", "Maxime")]), "Inconnue");
    }

    #[test]
    fn accept_language_choisit_la_bonne_langue() {
        // Le cas de la regle : le francais d'abord.
        assert_eq!(devine_langue("fr-FR,fr;q=0.9,en;q=0.8"), "fr");
        assert_eq!(devine_langue("en-US,en;q=0.9"), "en");
        // En-tete vide ou incomprehensible : langue par defaut.
        assert_eq!(devine_langue(""), LANGUE_PAR_DEFAUT);
        assert_eq!(devine_langue("   "), LANGUE_PAR_DEFAUT);
        assert_eq!(devine_langue("###"), LANGUE_PAR_DEFAUT);
        assert_eq!(devine_langue("*"), LANGUE_PAR_DEFAUT);
        // La qualite prime sur l'ordre de l'en-tete.
        assert_eq!(devine_langue("de;q=0.3,en;q=0.9"), "en");
        assert_eq!(devine_langue("de-DE;q=1.0,en;q=0.5"), "de");
        // q=0 veut dire « surtout pas ».
        assert_eq!(devine_langue("de;q=0,en;q=0.5"), "en");
        assert_eq!(devine_langue("de;q=0"), LANGUE_PAR_DEFAUT);
        // q illisible : on garde la langue, priorite par defaut.
        assert_eq!(devine_langue("it;q=abc,en;q=0.4"), "it");
        // Les espaces autour des elements ne genent pas.
        assert_eq!(devine_langue(" fr-CH , en ;q=0.5 "), "fr");

        // Parmi les langues du catalogue seulement.
        let mut c = Catalogue::nouveau("fr");
        c.ajouter("fr", "a", "A");
        c.ajouter("en", "a", "A");
        assert_eq!(c.devine_langue("de-DE,de;q=0.9,en;q=0.8"), "en");
        assert_eq!(c.devine_langue("es-ES"), "fr");
        assert_eq!(c.devine_langue(""), "fr");
        let seule = Catalogue::nouveau("it");
        assert_eq!(seule.devine_langue("en-US,en;q=0.9"), "it");
        assert_eq!(seule.devine_langue("it-IT"), "it");
    }

    #[test]
    fn codes_de_langue_equivalents() {
        let mut c = Catalogue::nouveau("FR");
        assert_eq!(c.langue_par_defaut(), "fr");
        c.ajouter("fr-FR", "Tva", "TVA");
        c.ajouter("fr_FR", "Devis", "Devis");
        // Une seule langue enregistree, quel que soit le code employe.
        assert_eq!(c.langues(), vec!["fr"]);
        assert_eq!(c.nb_cles("fr"), 2);
        assert_eq!(c.t("fr", "Tva"), "TVA");
        assert_eq!(c.t("fr-FR", "Devis"), "Devis");
        assert_eq!(c.t("FR_fr", "Tva"), "TVA");
        assert_eq!(c.t("Fr", "Devis"), "Devis");
        // Le repli marche aussi quand la langue par defaut a ete saisie en
        // majuscules.
        c.ajouter("en", "Tva", "VAT");
        assert_eq!(c.t("de", "Tva"), "TVA");
        assert_eq!(c.t("de", "Devis"), "Devis");
    }

    #[test]
    fn langues_triees_et_comptage() {
        let mut c = Catalogue::nouveau("fr");
        assert!(c.langues().is_empty());
        assert_eq!(c.nb_cles("fr"), 0);
        c.ajouter("en", "a", "A");
        c.ajouter("de", "a", "A");
        c.ajouter("fr", "a", "A");
        c.ajouter("fr", "b", "B");
        assert_eq!(c.langues(), vec!["de", "en", "fr"]);
        assert_eq!(c.nb_cles("fr"), 2);
        assert_eq!(c.nb_cles("en"), 1);
        assert_eq!(c.nb_cles("es"), 0);
        assert_eq!(c.nb_cles(""), 0);
        // Le chargement d'une langue la fait apparaitre.
        let mut c = Catalogue::nouveau("fr");
        c.depuis_texte("es", "Hola=Hola\nAdios=Adios\n");
        assert_eq!(c.langues(), vec!["es"]);
    }

    #[test]
    fn entrees_invalides_signalees() {
        let mut c = Catalogue::nouveau("fr");
        assert_eq!(c.ajouter_verifie("", "Cle", "x"), Err(Erreur::LangueVide));
        assert_eq!(c.ajouter_verifie("*", "Cle", "x"), Err(Erreur::LangueVide));
        assert_eq!(c.ajouter_verifie("fr", "   ", "x"), Err(Erreur::CleVide));
        assert_eq!(c.ajouter_verifie("fr", "", "x"), Err(Erreur::CleVide));
        assert!(c.langues().is_empty());
        assert!(c.ajouter_verifie("fr", "Cle", "x").is_ok());
        assert_eq!(c.t("fr", "Cle"), "x");

        // La variante sans Result jette l'entree fautive sans rien casser.
        c.ajouter("", "Cle2", "y");
        c.ajouter("fr", "", "y");
        assert_eq!(c.nb_cles("fr"), 1);

        // Un fichier charge dans une langue invalide ne retient rien.
        assert_eq!(c.depuis_texte("", "A=a\nB=b\n"), 0);
        assert_eq!(c.nb_cles("fr"), 1);

        // Erreur est un vrai type d'erreur, affichable.
        assert!(!Erreur::CleVide.to_string().is_empty());
        assert!(!Erreur::LangueVide.to_string().is_empty());
        let _: &dyn std::error::Error = &Erreur::LangueVide;
    }
}
