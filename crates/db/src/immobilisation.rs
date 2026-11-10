//! Immobilisations amortissables (`llx_asset`, `llx_asset_depreciation`).
//!
//! Une immobilisation, c'est une valeur entrée dans le patrimoine un jour donné
//! (la mise en service) et qui se consomme par morceaux sur une durée : chaque
//! exercice compte une **dotation** aux amortissements, et le cumul des dotations
//! ne doit jamais dépasser la valeur d'acquisition. Dolibarr stocke l'en-tête
//! dans `llx_asset` et le plan d'amortissement dans `llx_asset_depreciation` :
//! une ligne par exercice, portant la dotation de l'exercice et son cumulé.
//!
//! ## Pourquoi `plan()` existe, et pourquoi le dernier exercice porte l'arrondi
//!
//! La dotation linéaire vaut `valeur / durée`. Ce quotient tombe rarement juste :
//! 1 000 € sur 3 ans donnent 333,333… qui s'arrondit à 333,33. Écrire 333,33 sur
//! les trois lignes fait une somme de 999,99 : il reste un centime **jamais
//! amorti**, la valeur nette comptable ne tombe jamais sur zéro et le bilan garde
//! une immobilisation fantôme à un centime près, indéfiniment.
//!
//! D'où la règle unique de ce module : **les `n−1` premiers exercices portent la
//! dotation arrondie**, **le dernier porte tout le reliquat** (`valeur − somme des
//! dotations précédentes`). La somme des dotations égale donc la valeur
//! d'acquisition **au centime**, par construction et non par chance — c'est la
//! raison d'être de `plan()`, qui est pure et sans base, et c'est elle — pas le
//! SQL — qui garantit cette égalité.
//!
//! ## Décisions de modélisation
//!
//! - **Seul le linéaire est implémenté.** Un amortissement dégressif exige le
//!   coefficient fiscal (1,25 / 1,75 / 2,25 selon la durée), la bascule en
//!   linéaire sur le reliquat de la durée, puis le même traitement du reliquat
//!   d'arrondi. Tant que ce calcul n'est pas écrit et éprouvé, on ne déclare pas
//!   le variant : un `Methode::Degressif` qui produirait un plan linéaire
//!   mentirait sur la nature du plan, et le PDF Dolibarr afficherait la mauvaise
//!   nature comptable.
//! - La **date de clôture** de l'exercice `n` est la date de mise en service
//!   plus `n` ans, **au même jour**, ramené au dernier jour du mois quand ce jour
//!   n'existe pas : une mise en service au 29 février 2028 clôture au 28 février
//!   2029, 2030 et 2031, puis **de nouveau au 29 février 2032**. Le jour d'ancre
//!   est conservé en mémoire, sinon une immobilisation mise en service un 29, 30
//!   ou 31 décrocherait définitivement sur un jour plus court.
//! - `llx_asset_depreciation` **n'a pas de colonne `entity`** : c'est `llx_asset`
//!   qui porte l'entité. Toute lecture du plan passe donc par une jointure sur
//!   l'actif, sinon on lirait les annuités d'une autre société.
//! - `depreciation_date` est renseignée avec la **date de clôture** de
//!   l'exercice (à 00:00:00), pas avec l'instant d'écriture : c'est la date
//!   comptable de la dotation, celle que lit le grand livre.
//! - **Céder ne supprime rien.** Un actif cédé reste au registre : on pose
//!   `disposal_date` et `disposal_amount_ht`, on n'efface ni l'actif ni son plan.
//!   Les dotations passées sont des pièces comptables ; les effacer
//!   reviendrait à réécrire un bilan déjà déposé.
//! - Les montants sont des `double(24,8)` en base : ils sortent en
//!   `CAST(... AS CHAR)` puis passent par `Decimal::from_str`. Aucun total ne
//!   transite par un flottant, et les arrondis viennent de la crate `money`
//!   (demi hors de zéro, comme `price2num()` de Dolibarr), pas d'ici.

use std::error::Error;
use std::str::FromStr;

use money::round_money;
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

use crate::divers::suivant_ref;
use crate::reglement::{fin_de_mois, lire_date};

/// Longueur de `llx_asset.label` (`varchar(255)`).
const LABEL_MAX: usize = 255;

/// `depreciation_mode` d'un plan linéaire : c'est la valeur que Dolibarr écrit
/// et celle qu'il relit pour rattacher les lignes à l'option économique.
const MODE_LINEAIRE: &str = "linear";

/// Statut d'un actif en service : `llx_asset.status` vaut 1 (0 = brouillon).
const STATUT_EN_SERVICE: i32 = 1;

/// Méthode d'amortissement.
///
/// Voir la doc du module : le dégressif n'est pas déclaré tant qu'il n'est pas
/// calculé. Le `match` de [`PlanAmortissement::annuites`] rendra visible, le jour
/// où il arrivera, qu'il faudra aussi y toucher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Methode {
    /// Amortissement linéaire : `valeur / durée` par exercice.
    Lineaire,
}

impl Methode {
    /// Ce que cette méthode écrit dans `llx_asset_depreciation.depreciation_mode`.
    pub fn mode_comptable(self) -> &'static str {
        match self {
            Methode::Lineaire => MODE_LINEAIRE,
        }
    }
}

/// Une annuité calculée, sans identité en base : ce que rend [`plan`] avant
/// écriture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annuite {
    /// Rang de l'exercice, à partir de 1.
    pub exercice: u32,
    /// Date de clôture de l'exercice, `AAAA-MM-JJ`.
    pub date_cloture: String,
    /// Dotation de l'exercice. La dernière porte le reliquat d'arrondi.
    pub dotation: Decimal,
    /// Cumul des dotations depuis l'origine, cette annuité comprise. Vaut
    /// exactement la valeur d'acquisition sur la dernière ligne.
    pub cumule: Decimal,
}

/// Une annuité telle qu'elle est stockée dans `llx_asset_depreciation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnuiteEnBase {
    pub rowid: i64,
    /// Rang reconstitué à la lecture (l'ordre des dates), pas une colonne.
    pub exercice: u32,
    /// `depreciation_date`, `AAAA-MM-JJ` : la date de clôture de l'exercice.
    pub date: String,
    pub dotation: Decimal,
    pub cumule: Decimal,
}

/// Une immobilisation telle qu'affichée au registre : l'en-tête plus deux
/// valeurs calculées à la lecture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Immobilisation {
    pub rowid: i64,
    /// Colonne `ref` (le mot `ref` est réservé en Rust, d'où `reference`).
    pub reference: String,
    pub label: String,
    /// Valeur d'acquisition hors taxes (`acquisition_value_ht`).
    pub valeur: Decimal,
    /// Durée déduite du **nombre d'annuités en base**, pas d'une colonne : la
    /// durée n'est stockée nulle part, seul le plan la porte.
    pub duree_annees: u32,
    /// Valeur nette comptable : `valeur − cumul des dotations`.
    pub valeur_nette: Decimal,
    /// `date_start`, `AAAA-MM-JJ`.
    pub date_mise_en_service: String,
    /// Renseignée après cession. L'actif reste listé (voir la doc du module).
    pub date_cession: Option<String>,
}

/// Plan d'amortissement d'une immobilisation.
///
/// Les paramètres du plan, séparés de la valeur : la même grille (durée,
/// méthode) sert au calcul pur [`plan`] comme au calcul porté par `create()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanAmortissement {
    pub duree_annees: u32,
    pub methode: Methode,
}

impl PlanAmortissement {
    /// Plan linéaire sur `duree_annees` exercices.
    pub fn lineaire(duree_annees: u32) -> Self {
        Self {
            duree_annees,
            methode: Methode::Lineaire,
        }
    }

    /// Annuités du plan pour une valeur et une date de mise en service.
    ///
    /// Simple aiguillage vers [`plan`] : la valeur et la date ne font pas partie
    /// du plan, elles varient d'une immobilisation à l'autre pour une même durée.
    pub fn annuites(&self, valeur: Decimal, date_mise_en_service: &str) -> Vec<Annuite> {
        match self.methode {
            Methode::Lineaire => plan(valeur, self.duree_annees, date_mise_en_service),
        }
    }
}

/// Ce qu'il faut pour créer une immobilisation et son plan.
///
/// Les champs facultatifs à `None` restent `NULL` en base (jamais `0`, jamais
/// `0000-00-00`). Les colonnes `qty`, `acquisition_type` et `asset_type` ne sont
/// pas exposées : leurs valeurs par défaut (1, 0, 0) suffisent à une
/// immobilisation standard, et les exposer obligerait à inventer une
/// signification aux codes Dolibarr.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewImmobilisation {
    /// Libellé de la colonne `label`, obligatoire, 255 octets au plus.
    pub label: String,
    /// Valeur d'acquisition HT, strictement positive.
    pub valeur: Decimal,
    /// TVA récupérée sur l'achat, si elle est connue.
    pub recovered_vat: Option<Decimal>,
    /// Date d'acquisition, `AAAA-MM-JJ`. Vide, elle prend la date de mise en
    /// service : la colonne est NOT NULL et acquisition = mise en service est le
    /// cas courant d'un bien mis en service en sortie de facture.
    pub date_acquisition: String,
    /// Date de mise en service, `AAAA-MM-JJ`. C'est elle qui date le plan.
    pub date_mise_en_service: String,
    /// Durée d'amortissement en exercices, au moins 1.
    pub duree_annees: u32,
    /// Auteur de la création. `llx_asset.fk_user_creat` est NOT NULL **et** porte
    /// une clé étrangère vers `llx_user` : un identifiant inventé échouerait.
    pub fk_user_creat: i64,
    pub note_public: Option<String>,
    pub note_private: Option<String>,
}

// ---------------------------------------------------------------------------
// Calcul pur du plan d'amortissement
// ---------------------------------------------------------------------------

/// Plan d'amortissement linéaire.
///
/// Rend `duree_annees` annuités numérotées de 1 à n, clôturant **d'année en
/// année à partir de la mise en service**, même jour (ramené au dernier jour du
/// mois quand il n'existe pas). La somme des `dotation` égale `valeur` au
/// centime, le reliquat étant porté par la dernière annuité — c'est la raison
/// d'être de cette fonction, voir la doc du module.
///
/// La fonction ne panique jamais : elle rend un vecteur **vide** si
/// `duree_annees` vaut 0 (un plan de zéro exercice est vide, ce n'est pas une
/// erreur de calcul), si la valeur n'est pas strictement positive, ou si
/// `date_mise_en_service` n'est pas une date `AAAA-MM-JJ`. Le refus opposable à
/// l'utilisateur est prononcé par [`create`], qui valide ses entrées avant
/// d'appeler ici.
pub fn plan(valeur: Decimal, duree_annees: u32, date_mise_en_service: &str) -> Vec<Annuite> {
    if duree_annees == 0 || valeur <= Decimal::ZERO {
        return Vec::new();
    }
    let Some(ancre) = lire_date(date_mise_en_service) else {
        return Vec::new();
    };

    // Dotation de droit commun, arrondie une fois pour toutes : c'est la même
    // sur les n−1 premiers exercices, ce qui est exactement ce que dit un plan
    // linéaire (une dotation constante).
    let dotation_courante = round_money(valeur / Decimal::from(i64::from(duree_annees)));

    let mut cumule = Decimal::ZERO;
    // Une duree non bornee allouait `4 294 967 295` emplacements avant meme la
    // premiere iteration : l'allocation echoue et le processus meurt. Cent ans
    // couvrent tout plan d'amortissement reel.
    let duree_annees = duree_annees.clamp(1, 100);
    let mut annuites = Vec::with_capacity(duree_annees as usize);
    for exercice in 1..=duree_annees {
        let dotation = if exercice == duree_annees {
            // Dernier exercice : tout le reliquat, quelle que soit la dotation
            // calculée. C'est ce qui fait tomber la somme pile sur la valeur.
            valeur - cumule
        } else if cumule + dotation_courante > valeur {
            // Garde-fou : sur une valeur minuscule, la dotation arrondie peut
            // dépasser le reste à amortir. On s'arrête au reste, et les exercices
            // suivants ne portent plus rien plutôt qu'un montant négatif.
            valeur - cumule
        } else {
            dotation_courante
        };
        cumule += dotation;
        annuites.push(Annuite {
            exercice,
            date_cloture: date_cloture(ancre, exercice),
            dotation,
            cumule,
        });
    }
    annuites
}

/// Date de clôture du `annees_apres`-ième exercice après `ancre`.
///
/// Le jour est ancré sur celui de la mise en service et ramené au dernier jour
/// du mois cible quand il n'existe pas : partir du 29 février 2028 donne le
/// 28 février 2029, puis **le 29 février 2032** — pas le 28. Sans cette ancre,
/// l'actif décrocherait définitivement sur un jour plus court.
fn date_cloture(ancre: (i64, u32, u32), annees_apres: u32) -> String {
    let annee = ancre.0 + i64::from(annees_apres);
    let mois = ancre.1;
    // `fin_de_mois` de `reglement` gère les années bissextiles : on réutilise
    // cette arithmétique plutôt que d'en écrire une deuxième.
    let dernier_jour = fin_de_mois(&format!("{annee:04}-{mois:02}-01"))
        .and_then(|d| lire_date(&d).map(|(_, _, jour)| jour))
        .unwrap_or(28);
    let jour = ancre.2.min(dernier_jour);
    format!("{annee:04}-{mois:02}-{jour:02}")
}

// ---------------------------------------------------------------------------
// Lecture
// ---------------------------------------------------------------------------

/// Colonnes communes à `list` et `get`.
///
/// La durée et la valeur nette sont deux sous-requêtes corrélées : une jointure
/// avec `GROUP BY` obligerait à recalculer l'actif entier pour chaque annuité et
/// à filtrer les lignes dans un `HAVING`.
const COLS: &str = "a.rowid, a.ref, COALESCE(a.label, ''), \
     CAST(COALESCE(a.acquisition_value_ht, 0) AS CHAR), \
     DATE_FORMAT(a.date_start, '%Y-%m-%d'), \
     CAST(COALESCE((SELECT SUM(d.depreciation_ht) FROM llx_asset_depreciation d \
        WHERE d.fk_asset = a.rowid AND d.depreciation_date <= NOW()), 0) AS CHAR), \
     CAST((SELECT COUNT(*) FROM llx_asset_depreciation d \
        WHERE d.fk_asset = a.rowid) AS SIGNED), \
     DATE_FORMAT(a.disposal_date, '%Y-%m-%d')";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_immobilisation(row: &Row) -> Immobilisation {
    let valeur = round_money(opt_dec(row, 3).unwrap_or_default());
    let cumul = round_money(opt_dec(row, 5).unwrap_or_default());
    Immobilisation {
        rowid: opt_i(row, 0).unwrap_or(0),
        reference: opt_s(row, 1).unwrap_or_default(),
        label: opt_s(row, 2).unwrap_or_default(),
        valeur,
        duree_annees: opt_i(row, 6).unwrap_or(0) as u32,
        // Pas de bornage à zéro : une valeur nette négative signale une
        // sur-amortissement (plan écrit deux fois, dotation manuelle). La
        // masquer en silence cacherait précisément ce qu'il faut voir.
        valeur_nette: valeur - cumul,
        date_mise_en_service: opt_s(row, 4).unwrap_or_default(),
        date_cession: opt_s(row, 7),
    }
}

/// Immobilisations de l'entité, de la plus récente mise en service à la plus
/// ancienne. Les actifs cédés restent dans la liste.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Immobilisation>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {COLS} FROM llx_asset a \
         WHERE a.entity = :entity \
         ORDER BY a.date_start DESC, a.rowid DESC \
         LIMIT :limite OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_immobilisation).collect())
}

/// Une immobilisation précise, ou `None` si elle n'appartient pas à l'entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Immobilisation>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql =
        format!("SELECT {COLS} FROM llx_asset a WHERE a.entity = :entity AND a.rowid = :rowid");
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_immobilisation))
}

/// Plan d'amortissement persisté d'une immobilisation, dans l'ordre des dates.
///
/// La jointure sur `llx_asset` n'est pas décorative : `llx_asset_depreciation`
/// n'a pas d'`entity`, seul l'actif en porte une.
pub fn plan_en_base(
    pool: &Pool,
    entity: i64,
    fk_asset: i64,
) -> Result<Vec<AnnuiteEnBase>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT d.rowid, DATE_FORMAT(d.depreciation_date, '%Y-%m-%d'), \
                CAST(d.depreciation_ht AS CHAR), \
                CAST(d.cumulative_depreciation_ht AS CHAR) \
         FROM llx_asset_depreciation d \
         INNER JOIN llx_asset a ON a.rowid = d.fk_asset \
         WHERE a.entity = :entity AND d.fk_asset = :asset \
         ORDER BY d.depreciation_date ASC, d.rowid ASC",
        params! { "entity" => entity, "asset" => fk_asset },
    )?;
    Ok(rows
        .iter()
        .enumerate()
        .map(|(rang, row)| AnnuiteEnBase {
            rowid: opt_i(row, 0).unwrap_or(0),
            // L'exercice n'est pas stocké : il vient de l'ordre des dates, comme
            // le rang des échéances d'un prêt.
            exercice: rang as u32 + 1,
            date: opt_s(row, 1).unwrap_or_default(),
            dotation: round_money(opt_dec(row, 2).unwrap_or_default()),
            cumule: round_money(opt_dec(row, 3).unwrap_or_default()),
        })
        .collect())
}

/// Valeur nette comptable : valeur d'acquisition moins le cumul des dotations.
///
/// Le cumul est la **somme** des dotations, pas la dernière valeur de
/// `cumulative_depreciation_ht` : si une ligne du plan manquait ou était saisie
/// à la main, la somme reste juste alors que le dernier cumulé mentirait. Sur un
/// plan intégral les deux coïncident, par construction.
pub fn valeur_nette(pool: &Pool, entity: i64, fk_asset: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ligne: Option<Row> = conn.exec_first(
        "SELECT CAST(COALESCE(a.acquisition_value_ht, 0) AS CHAR), \
                CAST(COALESCE((SELECT SUM(d.depreciation_ht) FROM llx_asset_depreciation d \
                   WHERE d.fk_asset = a.rowid AND d.depreciation_date <= NOW()), 0) AS CHAR) \
         FROM llx_asset a WHERE a.entity = :entity AND a.rowid = :asset",
        params! { "entity" => entity, "asset" => fk_asset },
    )?;
    let Some(ligne) = ligne else {
        return Err(
            format!("l'immobilisation {fk_asset} n'existe pas dans l'entité {entity}").into(),
        );
    };
    let valeur = opt_dec(&ligne, 0).unwrap_or_default();
    let cumul = opt_dec(&ligne, 1).unwrap_or_default();
    Ok(round_money(valeur - cumul))
}

// ---------------------------------------------------------------------------
// Écriture
// ---------------------------------------------------------------------------

/// Crée une immobilisation et son plan d'amortissement, dans la même
/// transaction.
///
/// Refuse : un libellé vide ou de plus de 255 octets, une valeur nulle ou
/// négative, une TVA récupérée négative, une durée nulle, une date illisible ou
/// vide, et un auteur absent. Tous ces refus sont prononcés **avant** toute
/// écriture : un actif sans plan, ou un plan sans actif, serait une pièce
/// comptable inexploitable qu'il faudrait corriger à la main.
///
/// La référence est numérotée `IM0001`, `IM0002`… par entité. Elle est prise
/// **avant** l'ouverture de la transaction et sert aussi de racine aux
/// références d'annuités (`IM0007-1`, `IM0007-2`…), ce que la clé unique
/// `(fk_asset, depreciation_mode, ref)` accepte sans collision.
pub fn create(pool: &Pool, entity: i64, new: &NewImmobilisation) -> Result<i64, Box<dyn Error>> {
    let label = new.label.trim();
    if label.is_empty() {
        return Err("le libellé de l'immobilisation est obligatoire".into());
    }
    // On compte les **octets**, pas les caractères : c'est plus strict que la
    // colonne (255 caractères en utf8mb4) et cela laisse une marge aux libellés
    // accentués, qui pèsent deux octets par lettre accentuée.
    if label.len() > LABEL_MAX {
        return Err(format!(
            "le libellé de l'immobilisation dépasse {LABEL_MAX} octets (colonne label)"
        )
        .into());
    }
    let valeur = round_money(new.valeur);
    if valeur <= Decimal::ZERO {
        return Err("la valeur d'acquisition doit être strictement positive".into());
    }
    let tva_recuperee = match new.recovered_vat {
        None => None,
        Some(tva) => {
            let tva = round_money(tva);
            if tva < Decimal::ZERO {
                return Err("la TVA récupérée ne peut pas être négative".into());
            }
            Some(tva)
        }
    };
    if new.duree_annees == 0 {
        return Err("la durée d'amortissement doit être au moins 1 exercice".into());
    }
    let date_service = new.date_mise_en_service.trim();
    if lire_date(date_service).is_none() {
        return Err(format!(
            "la date de mise en service « {} » n'est pas une date AAAA-MM-JJ valide",
            new.date_mise_en_service
        )
        .into());
    }
    // Une date d'acquisition vide est un champ non renseigné, pas une date
    // invalide : les formulaires HTML renvoient toujours une chaîne, et la
    // colonne est NOT NULL.
    let date_acquisition = match new.date_acquisition.trim() {
        "" => date_service,
        d => d,
    };
    if lire_date(date_acquisition).is_none() {
        return Err(format!(
            "la date d'acquisition « {} » n'est pas une date AAAA-MM-JJ valide",
            new.date_acquisition
        )
        .into());
    }
    if new.fk_user_creat <= 0 {
        return Err("l'auteur de la création est obligatoire (fk_user_creat)".into());
    }

    let annuites = plan(valeur, new.duree_annees, date_service);
    if annuites.is_empty() {
        return Err("plan d'amortissement vide : vérifiez la valeur et la date".into());
    }

    // Numérotation hors transaction : `suivant_ref` ouvre sa propre connexion.
    let reference = suivant_ref(pool, "llx_asset", "ref", "IM", entity)?;

    let mut conn = pool.get_conn()?;
    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        // `qty`, `acquisition_type` et `asset_type` prennent leurs valeurs par
        // défaut (1, 0, 0). `date_creation` est NOT NULL sans défaut : NOW().
        // `not_depreciated` garde son défaut (false) : l'actif est amortissable,
        // c'est tout l'objet de ce module.
        "INSERT INTO llx_asset \
         (ref, entity, label, acquisition_value_ht, recovered_vat, \
          date_acquisition, date_start, fk_user_creat, date_creation, status, \
          note_public, note_private) \
         VALUES \
         (:reference, :entity, :label, :valeur, :tva_recuperee, \
          :date_acquisition, :date_start, :auteur, NOW(), :statut, \
          :note_public, :note_private)",
        params! {
            "reference" => reference.as_str(),
            "entity" => entity,
            "label" => label,
            "valeur" => valeur.to_string(),
            "tva_recuperee" => tva_recuperee.map(|t| t.to_string()),
            "date_acquisition" => date_acquisition,
            "date_start" => date_service,
            "auteur" => new.fk_user_creat,
            "statut" => STATUT_EN_SERVICE,
            "note_public" => new.note_public.as_deref(),
            "note_private" => new.note_private.as_deref(),
        },
    )?;
    let rowid = tx.last_insert_id().unwrap_or(0) as i64;

    for annuite in &annuites {
        // Une référence par exercice : `IM0007-1`, `IM0007-2`… Elle identifie la
        // pièce de dotation, et la clé unique (fk_asset, mode, ref) interdit
        // qu'un même exercice soit écrit deux fois.
        let piece = format!("{reference}-{}", annuite.exercice);
        tx.exec_drop(
            // Les comptes comptables restent NULL : les code comptable 681/281
            // dépendent du plan comptable de la société et du module de
            // comptabilité avancée, que ce noyau ne présume pas.
            "INSERT INTO llx_asset_depreciation \
             (fk_asset, depreciation_mode, ref, depreciation_date, depreciation_ht, \
              cumulative_depreciation_ht, accountancy_code_debit, accountancy_code_credit) \
             VALUES \
             (:asset, :mode, :piece, :date_cloture, :dotation, :cumule, NULL, NULL)",
            params! {
                "asset" => rowid,
                // Le mode vient de la méthode, pas d'une chaîne recopiée : si un
                // jour le dégressif existe, c'est ici que le plan devra choisir.
                "mode" => Methode::Lineaire.mode_comptable(),
                "piece" => piece.as_str(),
                "date_cloture" => annuite.date_cloture.as_str(),
                "dotation" => annuite.dotation.to_string(),
                "cumule" => annuite.cumule.to_string(),
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Cède une immobilisation : pose la date et le prix de cession.
///
/// Refuse une date illisible, un prix négatif, une cession **antérieure à la
/// mise en service** (on ne cède pas un bien avant de l'avoir) et une seconde
/// cession (elle écraserait la première pièce sans trace).
///
/// Un prix à 0 est licite : c'est une mise au rebut ou un don, et l'acte doit
/// exister même sans produit.
///
/// **Rien n'est supprimé.** L'actif et son plan restent au registre : les
/// dotations passées sont des écritures comptables, et un actif sorti du
/// patrimoine doit rester lisible avec sa date de sortie.
pub fn ceder(
    pool: &Pool,
    entity: i64,
    fk_asset: i64,
    date: &str,
    prix: Decimal,
) -> Result<(), Box<dyn Error>> {
    let date = date.trim();
    if lire_date(date).is_none() {
        return Err(
            format!("la date de cession « {date} » n'est pas une date AAAA-MM-JJ valide").into(),
        );
    }
    let prix = round_money(prix);
    if prix < Decimal::ZERO {
        return Err("le prix de cession ne peut pas être négatif".into());
    }

    let mut conn = pool.get_conn()?;
    let ligne: Option<Row> = conn.exec_first(
        "SELECT DATE_FORMAT(a.date_start, '%Y-%m-%d'), \
                DATE_FORMAT(a.disposal_date, '%Y-%m-%d') \
         FROM llx_asset a WHERE a.entity = :entity AND a.rowid = :asset",
        params! { "entity" => entity, "asset" => fk_asset },
    )?;
    let Some(ligne) = ligne else {
        return Err(
            format!("l'immobilisation {fk_asset} n'existe pas dans l'entité {entity}").into(),
        );
    };
    if let Some(deja) = opt_s(&ligne, 1) {
        return Err(format!(
            "l'immobilisation {fk_asset} est déjà cédée le {deja} : une cession ne se réécrit pas"
        )
        .into());
    }
    let mise_en_service = opt_s(&ligne, 0).unwrap_or_default();
    // Comparaison de chaînes : sur des dates ISO, l'ordre lexicographique est
    // l'ordre chronologique.
    if !mise_en_service.is_empty() && date < mise_en_service.as_str() {
        return Err(format!(
            "la cession du {date} précède la mise en service du {mise_en_service}"
        )
        .into());
    }

    conn.exec_drop(
        "UPDATE llx_asset SET disposal_date = :date, disposal_amount_ht = :prix, tms = NOW() \
         WHERE entity = :entity AND rowid = :asset AND disposal_date IS NULL",
        params! {
            "date" => date,
            "prix" => prix.to_string(),
            "entity" => entity,
            "asset" => fk_asset,
        },
    )?;
    if conn.affected_rows() == 0 {
        return Err(format!("l'immobilisation {fk_asset} est déjà cédée").into());
    }
    Ok(())
}
