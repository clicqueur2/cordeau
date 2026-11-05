//! La langue de l'interface.
//!
//! Le noyau est ecrit en francais : les textes des ecrans *sont* le francais.
//! Plutot que de renommer 935 chaines en cles inventees, on part du texte et on
//! cherche sa **traduction Dolibarr** : le catalogue `fr_FR` du projet donne,
//! pour chaque cle, le texte francais ; retourne, il donne pour chaque texte
//! francais la cle — et donc la traduction dans les trente langues que Dolibarr
//! fournit deja.
//!
//! Consequence : un ecran dont les textes existent dans Dolibarr est traduit
//! **sans traduire une ligne**. Ce qui n'y est pas reste en francais, ce qui est
//! le bon defaut — jamais une chaine vide, jamais une cle technique affichee.
//!
//! La langue est globale au processus (`DOLIBARR_LANGUE`, `fr_FR` par defaut).
//! Le choix par utilisateur demanderait de faire descendre la langue dans chaque
//! gestionnaire d'ecran ; ce n'est pas fait, et l'ecran ne fait pas semblant.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use i18n::Catalogue;

/// La langue par defaut, celle dans laquelle le noyau est ecrit.
pub const LANGUE_NOYAU: &str = "fr_FR";

/// Le catalogue charge, et l'index inverse du francais.
struct Table {
    catalogue: Catalogue,
    /// Texte francais -> ses cles Dolibarr (plusieurs possibles).
    inverse: BTreeMap<String, Vec<String>>,
    langue: String,
}

/// Le tableau charge une seule fois.
fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(charger)
}

/// Le dossier des fichiers de langue.
///
/// `DOLIBARR_LANGS` sinon le dossier vendu avec les sources. Le chemin par
/// defaut est **relatif au dossier de travail** : lance en service, il faut
/// poser la variable, sinon tout reste en francais (et le serveur le dit au
/// demarrage). Un dossier absent n'est pas une panne : on rend un catalogue
/// vide, et l'interface reste en francais.
fn racine() -> PathBuf {
    if let Ok(v) = std::env::var("DOLIBARR_LANGS") {
        let p = PathBuf::from(v);
        if p.exists() {
            return p;
        }
        // Une variable posee mais fausse ne doit pas rendre l'interface muette
        // en silence : on le dit, puis on cherche ailleurs.
        eprintln!(
            "DOLIBARR_LANGS pointe sur {} qui n'existe pas : catalogue cherche ailleurs",
            p.display()
        );
    }

    // Parmi les copies presentes, on prend celle qui porte le plus de langues.
    // Le dossier « src » livre avec les sources est tronque a trois langues
    // (fr, en, es) ; la reference complete en porte cent dix-neuf. Choisir par
    // defaut la copie tronquee faisait croire que Dolibarr ne fournit que trois
    // langues, alors que tout l'interet du catalogue est d'en avoir davantage.
    let candidats = [
        Path::new("_vendor")
            .join("dolibarr-ref")
            .join("htdocs")
            .join("langs"),
        Path::new("_vendor")
            .join("dolibarr-src")
            .join("htdocs")
            .join("langs"),
    ];
    candidats
        .into_iter()
        .filter(|c| c.exists())
        .max_by_key(|c| {
            i18n::dolibarr::langues_disponibles(c)
                .map(|l| l.len())
                .unwrap_or(0)
        })
        .unwrap_or_else(|| {
            Path::new("_vendor")
                .join("dolibarr-src")
                .join("htdocs")
                .join("langs")
        })
}

/// La langue demandee par l'environnement, `fr_FR` par defaut.
fn langue_demandee() -> String {
    std::env::var("DOLIBARR_LANGUE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| LANGUE_NOYAU.to_string())
}

fn charger() -> Table {
    charger_pour(&langue_demandee())
}

/// Charge un catalogue pour une langue donnee.
///
/// Prend la langue en parametre et non de l'environnement : l'ecran de
/// traduction mesure des langues qu'on n'utilise pas.
fn charger_pour(langue: &str) -> Table {
    let langue = langue.to_string();
    let mut catalogue = Catalogue::nouveau(LANGUE_NOYAU);
    let racine = racine();
    // On charge le francais (pour l'index inverse) puis la langue demandee.
    let mut demandees: Vec<&str> = vec![LANGUE_NOYAU];
    if langue != LANGUE_NOYAU {
        demandees.push(langue.as_str());
    }
    if let Err(e) = i18n::dolibarr::charger_langues(&mut catalogue, &racine, &demandees) {
        eprintln!("langues non chargees ({e}) : interface en francais");
    }
    // Nos propres libelles, par-dessus : la cle est le texte francais, donc ce
    // fichier ne fait que declarer ce que Dolibarr ne connait pas. Un dossier
    // absent n'est pas une erreur — on ne complete simplement rien.
    let noyau = std::env::var("DOLIBARR_LANGS_NOYAU")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new("langs").join("noyau"));
    if noyau.exists() {
        let _ = i18n::dolibarr::charger_langues(&mut catalogue, &noyau, &demandees);
    }
    let inverse = catalogue.index_inverse(LANGUE_NOYAU);

    Table {
        catalogue,
        inverse,
        langue,
    }
}

/// Le dossier des catalogues reellement utilise.
///
/// L'ecran de traduction l'affiche : sans ce chemin, « 3 langues disponibles »
/// ressemble a un bug alors que c'est le contenu du dossier livre.
pub fn racine_catalogue() -> PathBuf {
    racine()
}

/// La langue active.
pub fn langue() -> String {
    table().langue.clone()
}

/// Les langues proposees : celles de Dolibarr, plus les notres.
///
/// Fusionnees et triees : une langue presente des deux cotes ne doit pas
/// apparaitre deux fois dans une liste deroulante.
pub fn langues_disponibles() -> Vec<String> {
    let mut out: Vec<String> = i18n::dolibarr::langues_disponibles(&racine()).unwrap_or_default();
    let noyau = std::env::var("DOLIBARR_LANGS_NOYAU")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new("langs").join("noyau"));
    if noyau.exists() {
        if let Ok(autres) = i18n::dolibarr::langues_disponibles(&noyau) {
            out.extend(autres);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Ce qu'une langue sait faire d'un jeu de textes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bilan {
    /// Nombre de textes examines.
    pub total: usize,
    /// Nombre de textes qui changeraient en passant dans cette langue.
    pub traduits: usize,
}

impl Bilan {
    /// La part traduite, en pourcentage arrondi.
    pub fn pourcentage(&self) -> u32 {
        if self.total == 0 {
            return 0;
        }
        ((self.traduits * 100) / self.total) as u32
    }
}

/// Mesure une langue sur une liste de textes francais.
///
/// Le critere est celui de [`tr`] : un texte est « traduit » si la langue
/// rend autre chose que le texte d'origine. Une cle presente mais vide ne
/// compte donc pas — c'est exactement ce que verrait l'utilisateur.
pub fn couverture(langue: &str, textes: &[String]) -> Bilan {
    let table = charger_pour(langue);
    Bilan {
        total: textes.len(),
        traduits: textes
            .iter()
            .filter(|texte| traduire_avec(&table, langue, texte) != **texte)
            .count(),
    }
}

/// Les textes qui resteraient en francais dans cette langue, bornes a `max`.
///
/// Rendus tries : c'est une liste de travail, pas un echantillon.
pub fn manquants(langue: &str, textes: &[String], max: usize) -> Vec<String> {
    let table = charger_pour(langue);
    let mut out: Vec<String> = textes
        .iter()
        .filter(|texte| traduire_avec(&table, langue, texte) == **texte)
        .cloned()
        .collect();
    out.sort();
    out.truncate(max);
    out
}

/// La traduction d'un texte, avec un tableau fourni (testable sans fichier).
fn traduire_avec(table: &Table, langue: &str, texte: &str) -> String {
    if texte.trim().is_empty() || langue == LANGUE_NOYAU {
        return texte.to_string();
    }
    match table.inverse.get(texte) {
        // Plusieurs cles portent ce texte : on prend la premiere qui a une
        // traduction dans la langue visee. Sans ca, « Factures » sortait en
        // anglais par la cle `Bills` (non traduite) au lieu de `Invoices`.
        Some(cles) => {
            for cle in cles {
                if let Some(traduit) = table.catalogue.obtenir(langue, cle) {
                    return traduit.to_string();
                }
            }
            texte.to_string()
        }
        None => texte.to_string(),
    }
}

/// Traduit dans la langue active.
pub fn tr(texte: &str) -> String {
    let table = table();
    traduire_avec(table, &table.langue, texte)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_de_test() -> Table {
        let mut catalogue = Catalogue::nouveau(LANGUE_NOYAU);
        // Le catalogue Dolibarr : la cle est anglaise, le francais est la
        // traduction francaise.
        catalogue.ajouter("fr_FR", "Invoices", "Factures");
        catalogue.ajouter("fr_FR", "Proposals", "Propositions commerciales");
        catalogue.ajouter("en_US", "Invoices", "Invoices");
        catalogue.ajouter("en_US", "Proposals", "Proposals");
        let inverse = catalogue.index_inverse(LANGUE_NOYAU);
        Table {
            catalogue,
            inverse,
            langue: LANGUE_NOYAU.to_string(),
        }
    }

    #[test]
    fn en_francais_le_texte_est_rendu_tel_quel() {
        let t = table_de_test();
        assert_eq!(traduire_avec(&t, "fr_FR", "Factures"), "Factures");
    }

    #[test]
    fn en_anglais_le_texte_francais_trouve_sa_cle() {
        let t = table_de_test();
        assert_eq!(traduire_avec(&t, "en_US", "Factures"), "Invoices");
        assert_eq!(
            traduire_avec(&t, "en_US", "Propositions commerciales"),
            "Proposals"
        );
    }

    #[test]
    fn un_texte_absent_du_catalogue_reste_en_francais() {
        let t = table_de_test();
        assert_eq!(
            traduire_avec(&t, "en_US", "Clôture d'exercice"),
            "Clôture d'exercice"
        );
    }

    #[test]
    fn une_cle_sans_traduction_ne_s_affiche_jamais() {
        let mut t = table_de_test();
        // La cle existe en francais mais pas en anglais : `t` rendrait la cle.
        t.catalogue
            .ajouter("fr_FR", "OnlyFrench", "Seulement francais");
        t.inverse = t.catalogue.index_inverse(LANGUE_NOYAU);
        assert_eq!(
            traduire_avec(&t, "en_US", "Seulement francais"),
            "Seulement francais",
            "on garde le francais, jamais la cle technique"
        );
    }

    #[test]
    fn une_chaine_vide_reste_vide() {
        let t = table_de_test();
        assert_eq!(traduire_avec(&t, "en_US", ""), "");
        assert_eq!(traduire_avec(&t, "en_US", "   "), "   ");
    }

    #[test]
    fn le_pourcentage_ne_divise_jamais_par_zero() {
        assert_eq!(
            Bilan {
                total: 0,
                traduits: 0
            }
            .pourcentage(),
            0
        );
        assert_eq!(
            Bilan {
                total: 4,
                traduits: 1
            }
            .pourcentage(),
            25
        );
        assert_eq!(
            Bilan {
                total: 3,
                traduits: 3
            }
            .pourcentage(),
            100
        );
    }

    #[test]
    fn la_couverture_compte_les_textes_traduisibles() {
        let t = table_de_test();
        let textes = [
            "Factures".to_string(),
            "Propositions commerciales".to_string(),
            "Clôture d'exercice".to_string(),
        ];
        let n = textes
            .iter()
            .filter(|x| traduire_avec(&t, "en_US", x) != **x)
            .count();
        assert_eq!((n, textes.len()), (2, 3));
    }

    /// La mesure reelle, sur le catalogue Dolibarr vendu et sur l'inventaire
    /// des chaines de l'interface : combien de textes francais ont deja une
    /// traduction anglaise dans Dolibarr ?
    ///
    /// Le test ne juge pas le chiffre : il prouve que la chaine complete
    /// fonctionne (fichiers lus, index inverse construit, inventaire parcouru)
    /// et il affiche le compte.
    #[test]
    fn la_couverture_reelle_du_catalogue_dolibarr_est_mesuree() {
        let racine = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("racine du depot");
        let langs = racine.join("_vendor/dolibarr-src/htdocs/langs");
        if !langs.exists() {
            eprintln!("catalogue Dolibarr absent : mesure ignoree");
            return;
        }

        let mut catalogue = Catalogue::nouveau(LANGUE_NOYAU);
        if i18n::dolibarr::charger_langues(&mut catalogue, &langs, &[LANGUE_NOYAU, "en_US"])
            .is_err()
        {
            eprintln!("catalogue illisible : mesure ignoree");
            return;
        }
        // Nos propres libelles comptent aussi : ce sont ceux que Dolibarr ne
        // connait pas.
        let noyau = racine.join("langs/noyau");
        if noyau.exists() {
            let _ =
                i18n::dolibarr::charger_langues(&mut catalogue, &noyau, &[LANGUE_NOYAU, "en_US"]);
        }
        let table = Table {
            inverse: catalogue.index_inverse(LANGUE_NOYAU),
            catalogue,
            langue: LANGUE_NOYAU.to_string(),
        };

        let inventaire = i18n::ecrans::inventaire(racine).unwrap_or_default();
        if inventaire.is_empty() {
            eprintln!("inventaire vide : mesure ignoree");
            return;
        }
        let textes: Vec<String> = inventaire.iter().map(|c| c.texte.clone()).collect();
        let traduits = textes
            .iter()
            .filter(|x| traduire_avec(&table, "en_US", x) != **x)
            .count();

        println!(
            "couverture Dolibarr : {traduits} / {} textes de l'interface ont une traduction anglaise",
            textes.len()
        );
        assert!(!table.inverse.is_empty(), "l'index inverse est vide");
    }
}
