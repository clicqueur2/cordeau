//! Inventaire des chaines francaises ecrites en dur dans l'interface web.
//!
//! Ce module ne traduit rien : il **compte**. Avant de remplacer les textes par
//! des cles, il faut savoir combien il y en a et ou ils sont. `inventaire` passe
//! au crible `crates/web/src/*.rs` et rend la liste des litteraux qui
//! ressemblent a du francais ; `rapport_texte` en fait un tableau lisible ;
//! `resume` donne la densite par fichier.
//!
//! Sans cet inventaire, « traduire l'interface » reste une intention. Avec lui,
//! c'est un plan chiffre : un nombre, des fichiers, des lignes.
//!
//! # Ce qui est reconnu
//!
//! Un litteral est retenu quand il a au moins 4 caracteres, au moins une
//! lettre, et un **indice de francais** :
//!
//! 1. un accent (`e a c u o i e...` accentues, majuscules comprises) ;
//! 2. un mot d'une liste fermee de mots-outils (`le`, `la`, `des`, `pour`,
//!    `avec`, `dans`, `est`, `votre`...) ;
//! 3. *(extension assumee, voir plus bas)* un **libelle alphabetique** : un
//!    texte fait uniquement de lettres, espaces, apostrophes et points, qui
//!    compte au moins deux mots (`Total HT`, `Se connecter`, `devis
//!    introuvable`) ou un seul mot capitalise (`Stock`, `Avoirs`, `Contrats`).
//!    Sans cette troisieme regle, les libelles courts — les plus visibles de
//!    l'ecran — echapperaient tous a l'inventaire : `Aucun devis en attente.`
//!    n'a ni accent ni mot-outil, et `Tiers`, `Factures`, `Montant` non plus.
//!    Elle reste etroite : un compose a trait d'union sans espace
//!    (`Content-Type`, `dolibarr-rs`) est un identifiant, et les sigles
//!    (`POST`, `HTML`, `JSON`...) et tetes anglaises (`Content`, `Bearer`...)
//!    sont ecartes par liste noire.
//!
//! # Ce qui est rejete
//!
//! - les fragments CSS : tout litteral contenant `;` ou `#`, ou une unite `px`
//!   (`width:130px`) ;
//! - les URL (`://`, `http`, `www.`) et les chemins (`/factures`, `text/css`) ;
//! - les identifiants (`oui_non`, `fk_soc`) et les chaines de moins de 4
//!   caracteres (`Non`, `Oui`, `1`, `post`) ;
//! - les commentaires `//` et `/* */` du code : le lexeur les saute, donc un
//!   exemple entre guillemets dans un commentaire n'est pas compte.
//!
//! # Les limites, dites franchement
//!
//! - **Faux negatifs.** Les sigles de moins de 4 caracteres (`TVA`, `HT`,
//!   `RIB`) echappent au filtre, ainsi que `Ref.` (3 lettres une fois le point
//!   ote) et `E-mail` (trait d'union sans espace, ecarte comme identifiant).
//! - **Faux positifs.** Une valeur technique capitalisee inconnue des listes
//!   noires serait prise pour un libelle : c'est le cas de `DOLAPIKEY`, nom
//!   d'en-tete HTTP, qui figure dans l'inventaire. Les cles JSON francaises
//!   (`"produit"`, `"ville"`) sont ecartees par les regles 1 et 2.
//! - Les litteraux **multi-lignes** en guillemets simples ne sont lus que ligne
//!   par ligne : une phrase coupee sur deux lignes est vue comme un fragment
//!   (le premier morceau seulement), donc sous-comptee. C'est le cas de
//!   `main.rs:3886`. En revanche une chaine **brute** ouverte (`r#"`) est suivie
//!   d'une ligne a l'autre et sautee en entier : c'est ainsi que le bloc CSS de
//!   `ui.rs` (et sa police `"Segoe UI"`) ne pollue pas l'inventaire.
//! - La detection de `mod tests` est textuelle : des la premiere ligne
//!   contenant `mod tests`, **tout le reste du fichier** est ignore, meme si du
//!   code de production suivait (ce qui n'arrive pas dans ce depot).
//! - La deduplication se fait sur le texte seul : `"Supprimer"` vu dans
//!   `pages.rs` et dans `main.rs` ne compte qu'une fois, a sa premiere
//!   occurrence. Le total est donc un nombre de **cles**, pas d'occurrences.

use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Nombre maximal d'entrees rendues : au-dela, le rapport n'aide plus.
pub const MAX_ENTREES: usize = 5000;

/// Nombre minimal de caracteres d'un litteral pour etre examine.
const LONGUEUR_MIN: usize = 4;

/// Accents qui trahissent le francais. Les majuscules comptent aussi : un
/// libelle commence souvent par une majuscule (`Éxpéditions`).
const ACCENTS: &[char] = &[
    'é', 'è', 'ê', 'ë', 'à', 'â', 'ç', 'ù', 'û', 'ô', 'î', 'ï', 'œ', 'æ', 'É', 'È', 'Ê', 'Ë', 'À',
    'Â', 'Ç', 'Ù', 'Û', 'Ô', 'Î', 'Ï', 'Œ', 'Æ',
];

/// Mots-outils : leur presence suffit a identifier une phrase francaise, meme
/// sans accent (`Saisie des reglements`).
const MOTS_OUTILS: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "du", "de", "pour", "avec", "sans", "sur", "sous",
    "dans", "par", "et", "ou", "est", "sont", "doit", "etre", "cette", "ces", "votre", "vos",
    "notre", "nos", "au", "aux",
];

/// Sigles techniques qui ne sont pas des libelles, meme capitalises.
const SIGLES: &[&str] = &[
    "POST", "GET", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "HTML", "CSS", "JSON", "HTTP",
    "HTTPS", "SQL", "UTF", "PDF", "CSV", "XML", "UUID", "TRUE", "FALSE", "NULL", "NONE", "UTF8",
];

/// Premiers mots anglais ou techniques qui, capitalises, ne sont pas des
/// libelles francais (`Content-Type`, `Bearer ...`).
const MOTS_TECHNIQUES: &[&str] = &[
    "content",
    "accept",
    "bearer",
    "basic",
    "cookie",
    "cache",
    "charset",
    "authorization",
    "user",
    "none",
    "true",
    "false",
    "null",
];

/// Une chaine francaise reperee dans un fichier de l'interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chaine {
    /// Nom du fichier, sans chemin : `pages.rs`.
    pub fichier: String,
    /// Numero de ligne, a partir de 1.
    pub ligne: u32,
    /// Le texte du litteral, sans les guillemets.
    pub texte: String,
}

/// Recense les chaines francaises en dur sous `racine/crates/web/src/*.rs`.
///
/// Les fichiers sont lus par ordre alphabetique, puis ligne a ligne. Les
/// doublons de texte sont ecartes (premiere occurrence gardee) et le resultat
/// est borne a [`MAX_ENTREES`].
///
/// # Erreurs
///
/// Rend une erreur si `racine`, `racine/crates/web/src` ou un fichier est
/// illisible. Un dossier absent n'est **pas** assimile a un inventaire vide :
/// zero chaine silencieux ferait croire que l'interface est deja traduite.
pub fn inventaire(racine: &Path) -> Result<Vec<Chaine>, Box<dyn Error>> {
    let dossier = racine.join("crates").join("web").join("src");

    let mut fichiers: Vec<PathBuf> = Vec::new();
    for entree in fs::read_dir(&dossier)? {
        let chemin = entree?.path();
        if chemin.extension().and_then(|e| e.to_str()) == Some("rs") {
            fichiers.push(chemin);
        }
    }
    fichiers.sort();

    let mut chaines: Vec<Chaine> = Vec::new();
    let mut vus: HashSet<String> = HashSet::new();

    'fichiers: for chemin in &fichiers {
        let contenu = fs::read_to_string(chemin)?;
        let nom = chemin
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("inconnu")
            .to_string();

        let mut etat = Etat::default();
        for (index, ligne) in contenu.lines().enumerate() {
            // Approximation assumee : des `mod tests`, tout le reste du fichier
            // est du test et ne s'affiche jamais a l'ecran.
            if ligne.contains("mod tests") {
                break;
            }

            let mut litteraux: Vec<String> = Vec::new();
            extraire_litteraux(ligne, &mut etat, &mut litteraux);

            for texte in litteraux {
                if !est_francais_probable(&texte) {
                    continue;
                }
                if !vus.insert(texte.clone()) {
                    continue;
                }
                let ligne_sur: u32 = u32::try_from(index + 1).unwrap_or(u32::MAX);
                chaines.push(Chaine {
                    fichier: nom.clone(),
                    ligne: ligne_sur,
                    texte,
                });
                if chaines.len() >= MAX_ENTREES {
                    break 'fichiers;
                }
            }
        }
    }

    Ok(chaines)
}

/// Tableau texte : fichier, ligne, texte, trie par fichier puis par ligne.
pub fn rapport_texte(chaines: &[Chaine]) -> String {
    let mut tri: Vec<&Chaine> = chaines.iter().collect();
    tri.sort_by(|a, b| a.fichier.cmp(&b.fichier).then(a.ligne.cmp(&b.ligne)));

    let largeur = tri
        .iter()
        .map(|c| c.fichier.chars().count())
        .max()
        .unwrap_or(0)
        .max("fichier".len());

    let mut sortie = String::new();
    sortie.push_str(&format!("{} chaines distinctes\n\n", chaines.len()));
    sortie.push_str(&format!(
        "{:<largeur$}  {:>6}  {}\n",
        "fichier",
        "ligne",
        "texte",
        largeur = largeur
    ));
    sortie.push_str(&"-".repeat(largeur + 6 + 4 + 40));
    sortie.push('\n');

    for c in tri {
        sortie.push_str(&format!(
            "{:<largeur$}  {:>6}  {}\n",
            c.fichier,
            c.ligne,
            c.texte,
            largeur = largeur
        ));
    }

    sortie
}

/// Densite par fichier : `(fichier, nombre de chaines)`, du plus dense au moins
/// dense. A egalite, l'ordre alphabetique tranche (sortie stable).
pub fn resume(chaines: &[Chaine]) -> Vec<(String, usize)> {
    let mut comptes: Vec<(String, usize)> = Vec::new();
    for c in chaines {
        match comptes.iter_mut().find(|(f, _)| f == &c.fichier) {
            Some((_, n)) => *n += 1,
            None => comptes.push((c.fichier.clone(), 1)),
        }
    }
    comptes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    comptes
}

/// Ce que le lexeur doit savoir d'une ligne a l'autre : un commentaire de bloc
/// ouvert reste ouvert, une chaine brute ouverte reste ouverte.
#[derive(Default)]
struct Etat {
    /// Vrai entre `/*` et `*/`.
    dans_bloc: bool,
    /// Nombre de `#` d'une chaine brute ouverte (`r#"` en attente de `"#`).
    /// `ui.rs` en contient une : tout le bloc CSS.
    chaine_brute: Option<usize>,
}

/// Extrait les litteraux d'une ligne de Rust.
///
/// Le parcours est un vrai petit lexeur, pas une recherche de guillemets : il
/// saute les commentaires `//` et `/* */` (y compris ceux ouverts sur une ligne
/// precedente, via `etat`), les chaines brutes `r#"..."#` — y compris quand
/// elles couvrent plusieurs lignes, comme le bloc CSS de `ui.rs` —, les
/// caracteres `'x'` et les echappements. Sans cela, `// il ecrit "bonjour"`
/// compterait un faux texte, `"a \"b\""` serait coupe au milieu, et la police
/// `"Segoe UI"` du CSS passerait pour un libelle.
fn extraire_litteraux(ligne: &str, etat: &mut Etat, sortie: &mut Vec<String>) {
    let c: Vec<char> = ligne.chars().collect();
    let mut i = 0;

    // On sort d'une chaine brute ouverte a la ligne precedente.
    if let Some(dieses) = etat.chaine_brute {
        match fin_chaine_brute(&c, 0, dieses) {
            Some(fin) => {
                etat.chaine_brute = None;
                i = fin;
            }
            None => return,
        }
    }

    while i < c.len() {
        if etat.dans_bloc {
            if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                etat.dans_bloc = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        if c[i] == '/' && c.get(i + 1) == Some(&'/') {
            return;
        }
        if c[i] == '/' && c.get(i + 1) == Some(&'*') {
            etat.dans_bloc = true;
            i += 2;
            continue;
        }

        // Chaine brute : r"..." ou r#"..."# (nombre de # quelconque).
        if c[i] == 'r' && (i == 0 || !est_ident(c[i - 1])) {
            let mut j = i + 1;
            let mut dieses = 0;
            while j < c.len() && c[j] == '#' {
                dieses += 1;
                j += 1;
            }
            if j < c.len() && c[j] == '"' {
                let debut = j + 1;
                match fin_chaine_brute(&c, debut, dieses) {
                    Some(fin) => {
                        let dernier = fin - 1 - dieses;
                        let texte: String = c[debut..dernier].iter().collect();
                        sortie.push(texte);
                        i = fin;
                        continue;
                    }
                    None => {
                        // La chaine continue a la ligne suivante : on retient
                        // l'etat et on abandonne ce fragment de tete (le bloc
                        // CSS de `ui.rs` commence ainsi).
                        etat.chaine_brute = Some(dieses);
                        return;
                    }
                }
            }
        }

        if c[i] == '"' {
            let mut k = i + 1;
            let mut texte = String::new();
            let mut ferme = false;
            while k < c.len() {
                if c[k] == '\\' {
                    if let Some(suivant) = c.get(k + 1) {
                        texte.push(*suivant);
                        k += 2;
                        continue;
                    }
                }
                if c[k] == '"' {
                    ferme = true;
                    break;
                }
                texte.push(c[k]);
                k += 1;
            }
            sortie.push(texte);
            i = if ferme { k + 1 } else { c.len() };
            continue;
        }

        // Caractere litteral : '"' ou '\'' ne doit pas etre pris pour un
        // guillemet ouvrant. Les durees de vie ('a) sont laissees passer.
        if c[i] == '\'' {
            if c.get(i + 2) == Some(&'\'') && c.get(i + 1) != Some(&'\\') {
                i += 3;
                continue;
            }
            if c.get(i + 1) == Some(&'\\') {
                if let Some(fin) = (i + 2..c.len()).find(|&k| c[k] == '\'') {
                    i = fin + 1;
                    continue;
                }
            }
        }

        i += 1;
    }
}

fn est_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Index juste apres la fermeture `"###` d'une chaine brute, en partant de
/// `depart`. `None` si elle ne se ferme pas sur cette ligne.
fn fin_chaine_brute(c: &[char], depart: usize, dieses: usize) -> Option<usize> {
    let mut k = depart;
    while k < c.len() {
        if c[k] == '"' {
            let mut suite = 0;
            while suite < dieses && c.get(k + 1 + suite) == Some(&'#') {
                suite += 1;
            }
            if suite == dieses {
                return Some(k + 1 + dieses);
            }
        }
        k += 1;
    }
    None
}

/// Decide si un litteral ressemble a du francais destine a l'ecran.
fn est_francais_probable(texte: &str) -> bool {
    if texte.chars().count() < LONGUEUR_MIN {
        return false;
    }
    if !texte.chars().any(char::is_alphabetic) {
        return false;
    }

    // Fragments CSS et couleurs.
    if texte.contains(';') || texte.contains('#') || contient_unite_px(texte) {
        return false;
    }
    // URL, chemins, types MIME.
    if texte.contains("://")
        || texte.starts_with("http")
        || texte.starts_with("www.")
        || (texte.contains('/') && !texte.contains(' '))
    {
        return false;
    }
    // Identifiants du code : fk_soc, oui_non, total_ht.
    if texte.contains('_') && !texte.contains(' ') {
        return false;
    }
    // Valeur d'attribut HTML ou etat interne, toujours en minuscules collées.
    if texte.chars().all(|c| !c.is_whitespace()) && texte.contains(':') {
        return false;
    }

    if texte.chars().any(|c| ACCENTS.contains(&c)) {
        return true;
    }

    let mots: Vec<String> = texte
        .split(|c: char| !c.is_alphabetic())
        .filter(|m| !m.is_empty())
        .map(str::to_lowercase)
        .collect();
    if mots.iter().any(|m| MOTS_OUTILS.contains(&m.as_str())) {
        return true;
    }

    // Extension assumee : un libelle alphabetique, c'est-a-dire un texte fait
    // uniquement de lettres, espaces, apostrophes et points. Deux formes le
    // designent : plusieurs mots (`devis introuvable`, `Total HT`,
    // `Se connecter`), ou un seul mot capitalise (`Contrats`, `Stock`). Un
    // compose a trait d'union sans espace (`Content-Type`, `dolibarr-rs`) est un
    // identifiant, pas un libelle ; les sigles et les tetes anglaises sont
    // ecartes par liste noire.
    let nu = texte.trim().trim_end_matches(['.', ':', ',']).trim();
    let alphanumerique = nu
        .chars()
        .all(|c| c.is_alphabetic() || c == ' ' || c == '\'' || c == '.' || c == '-');
    if !alphanumerique || nu.contains('#') || (nu.contains('-') && !nu.contains(' ')) {
        return false;
    }

    let plusieurs_mots = nu.split_whitespace().count() >= 2;
    let mot_unique_capitalise =
        nu.chars().all(char::is_alphabetic) && nu.chars().next().is_some_and(char::is_uppercase);
    if !plusieurs_mots && !mot_unique_capitalise {
        return false;
    }

    let haut = nu.to_uppercase();
    if haut == nu && SIGLES.contains(&haut.as_str()) {
        return false;
    }
    let tete = nu.split_whitespace().next().unwrap_or("").to_lowercase();
    if MOTS_TECHNIQUES.contains(&tete.as_str()) {
        return false;
    }

    true
}

/// Vrai si le texte porte une unite CSS en pixels (`130px`, `2.5px`).
fn contient_unite_px(texte: &str) -> bool {
    let c: Vec<char> = texte.chars().collect();
    let mut i = 0;
    while i + 1 < c.len() {
        if c[i] == 'p' && c[i + 1] == 'x' {
            let avant = if i == 0 { None } else { Some(c[i - 1]) };
            let avant_ok =
                matches!(avant, Some(a) if a.is_ascii_digit() || a == '.' || a == '-' || a == ')');
            let apres_ok = c.get(i + 2).is_none_or(|a| !a.is_alphanumeric());
            if avant_ok && apres_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Racine du depot : `crates/i18n` remonte de deux crans.
    fn racine_depot() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("racine du depot introuvable")
            .to_path_buf()
    }

    #[test]
    fn inventaire_reel_borne_entre_200_et_5000() {
        let chaines = inventaire(&racine_depot()).expect("inventaire de crates/web/src");
        assert!(
            chaines.len() > 200,
            "une interface entiere doit rendre plus de 200 chaines, trouve {}",
            chaines.len()
        );
        assert!(
            chaines.len() < MAX_ENTREES,
            "le plafond ne doit pas etre atteint, trouve {}",
            chaines.len()
        );
    }

    #[test]
    fn pages_est_parmi_les_plus_denses() {
        let chaines = inventaire(&racine_depot()).expect("inventaire");
        let r = resume(&chaines);
        let position = r
            .iter()
            .position(|(f, _)| f == "pages.rs")
            .expect("pages.rs doit figurer dans le resume");
        // `main.rs` est plus gros en octets, donc il peut passer devant ;
        // `pages.rs` doit au moins etre sur le podium.
        assert!(
            position <= 1,
            "pages.rs doit etre en tete ou presque, il est en position {} : {:?}",
            position + 1,
            &r[..3.min(r.len())]
        );
        let n = r[position].1;
        assert!(
            n > 200,
            "pages.rs doit porter l'essentiel des chaines, trouve {n}"
        );
    }

    #[test]
    fn aucun_fragment_css_dans_les_resultats() {
        let chaines = inventaire(&racine_depot()).expect("inventaire");
        for c in &chaines {
            assert!(
                !c.texte.contains(';'),
                "point-virgule CSS dans {}:{} : {:?}",
                c.fichier,
                c.ligne,
                c.texte
            );
            assert!(
                !c.texte.contains('#'),
                "diese CSS dans {}:{} : {:?}",
                c.fichier,
                c.ligne,
                c.texte
            );
            assert!(
                !c.texte.contains("px"),
                "unite px dans {}:{} : {:?}",
                c.fichier,
                c.ligne,
                c.texte
            );
        }
    }

    #[test]
    fn fichier_temporaire_trois_francais_deux_css() {
        let base = std::env::temp_dir().join(format!(
            "i18n-ecrans-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let src = base.join("crates").join("web").join("src");
        fs::create_dir_all(&src).expect("dossier temporaire");
        let fichier = src.join("essai.rs");
        let contenu = concat!(
            "pub fn a() {\n",
            "    let _ = \"Le total des factures est faux\";\n",
            "    let _ = \"Aucun reglement trouvé\";\n",
            "    let _ = \"Stock\";\n",
            "    let _ = \"width:130px;border:1px solid #e6e2dc\";\n",
            "    let _ = \"color:red;\";\n",
            "}\n",
        );
        fs::write(&fichier, contenu).expect("ecriture du fichier d'essai");

        let chaines = inventaire(&base).expect("inventaire du dossier temporaire");
        fs::remove_dir_all(&base).expect("nettoyage");

        assert_eq!(chaines.len(), 3, "attendu 3 chaines, trouve {chaines:?}");
        assert_eq!(chaines[0].fichier, "essai.rs");
        assert_eq!(chaines[0].ligne, 2);
        assert!(chaines.iter().all(|c| c.fichier == "essai.rs"));
    }

    #[test]
    fn chaine_brute_multilignes_ignoree() {
        let base = std::env::temp_dir().join(format!(
            "i18n-ecrans-brut-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let src = base.join("crates").join("web").join("src");
        fs::create_dir_all(&src).expect("dossier temporaire");
        let contenu = concat!(
            "const CSS: &str = r#\"\n",
            "body { font:15px/1.55 \"Segoe UI\", sans-serif; }\n",
            "header .marque { color:#8a5a2b; }\n",
            "\"#;\n",
            "pub fn a() {\n",
            "    let _ = \"Le vrai texte des tiers\";\n",
            "}\n",
        );
        fs::write(src.join("brut.rs"), contenu).expect("ecriture");

        let chaines = inventaire(&base).expect("inventaire");
        fs::remove_dir_all(&base).expect("nettoyage");

        assert_eq!(
            chaines.len(),
            1,
            "le bloc brut doit etre saute : {chaines:?}"
        );
        assert_eq!(chaines[0].texte, "Le vrai texte des tiers");
        assert_eq!(chaines[0].ligne, 6);
    }

    #[test]
    fn racine_inexistante_est_une_erreur() {
        let base = std::env::temp_dir().join(format!(
            "i18n-ecrans-absent-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let resultat = inventaire(&base);
        assert!(
            resultat.is_err(),
            "une racine absente doit rendre une erreur, pas un inventaire vide"
        );
    }

    #[test]
    fn rapport_et_resume_sont_tries() {
        let chaines = vec![
            Chaine {
                fichier: "b.rs".to_string(),
                ligne: 9,
                texte: "Deuxieme".to_string(),
            },
            Chaine {
                fichier: "a.rs".to_string(),
                ligne: 42,
                texte: "Premiere".to_string(),
            },
            Chaine {
                fichier: "a.rs".to_string(),
                ligne: 7,
                texte: "Troisieme".to_string(),
            },
        ];

        let rapport = rapport_texte(&chaines);
        let lignes: Vec<&str> = rapport.lines().collect();
        assert!(rapport.starts_with("3 chaines distinctes"));
        assert!(lignes[2].starts_with("fichier"));
        // a.rs ligne 7 avant a.rs ligne 42, puis b.rs.
        assert!(lignes[4].starts_with("a.rs"));
        assert!(lignes[4].contains("Troisieme"));
        assert!(lignes[5].starts_with("a.rs"));
        assert!(lignes[5].contains("Premiere"));
        assert!(lignes[6].starts_with("b.rs"));

        let r = resume(&chaines);
        assert_eq!(r[0], ("a.rs".to_string(), 2));
        assert_eq!(r[1], ("b.rs".to_string(), 1));
    }

    #[test]
    fn les_filtres_rejettent_ce_qu_il_faut() {
        assert!(!est_francais_probable("post"));
        assert!(!est_francais_probable("oui_non"));
        assert!(!est_francais_probable("/factures"));
        assert!(!est_francais_probable("https://exemple.fr/page"));
        assert!(!est_francais_probable("width:130px"));
        assert!(!est_francais_probable("#e6e2dc"));
        assert!(!est_francais_probable("GET"));
        assert!(est_francais_probable("Total des factures"));
        assert!(est_francais_probable("Expéditions"));
        assert!(est_francais_probable("Contrats"));
    }
}
