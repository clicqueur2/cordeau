//! Prêts bancaires (`llx_loan`, `llx_loan_schedule`, `llx_payment_loan`).
//!
//! Un prêt, c'est un capital emprunté le jour de la signature, un taux annuel,
//! un nombre d'échéances, et un échéancier : la suite des mensualités qui
//! soldent le capital, intérêts compris. Dolibarr stocke l'en-tête dans
//! `llx_loan` et les échéances dans `llx_loan_schedule` ; le règlement d'une
//! échéance crée une ligne dans `llx_payment_loan` et pointe dessus par
//! `llx_loan_schedule.fk_payment_loan`.
//!
//! ## Pourquoi `echeancier()` existe, et pourquoi la dernière échéance porte l'arrondi
//!
//! La mensualité d'un prêt à taux fixe se calcule par la formule classique
//! `m = C · i / (1 − (1 + i)^−n)`. Ce nombre est presque toujours irrationnel :
//! arrondi au centime, il ne divise pas exactement le capital. Si l'on écrit
//! `capital = mensualité − intérêts` sur **toutes** les lignes, la somme des
//! parts de capital vaut 10 000,01 ou 9 999,99 au lieu de 10 000,00 : il reste
//! des centimes flottants, le capital restant dû ne tombe jamais sur zéro, et le
//! prêt paraît éternel dans la fiche Dolibarr.
//!
//! D'où la règle unique de ce module : **les `n−1` premières échéances portent
//! la mensualité arrondie** (part de capital = mensualité − intérêts du mois),
//! **et la dernière échéance porte tout le reliquat** (`capital − somme des
//! parts précédentes`). La somme des parts de capital égale donc le capital
//! **au centime**, par construction et non par chance. C'est la raison d'être de
//! `echeancier()` : elle est pure, sans base, et c'est elle — pas le SQL — qui
//! garantit cette égalité.
//!
//! ## Décisions de modélisation
//!
//! - `llx_loan_schedule` **n'a pas de colonne `entity`** : c'est `llx_loan` qui
//!   porte l'entité. Toute lecture d'échéance passe donc par une jointure sur le
//!   prêt, sinon on lirait les échéances d'une autre société.
//! - `fk_payment_loan` référence la table `llx_payment_loan`, qui existe bien au
//!   schéma 24.0.1 (mêmes colonnes que l'échéance, moins `fk_payment_loan`
//!   elle-même). `regler()` y insère le paiement puis pointe l'échéance dessus :
//!   on n'invente aucune colonne, on suit le modèle de Dolibarr.
//! - `llx_loan.capital_position` est un **reflet** du capital restant dû, mis à
//!   jour à chaque règlement pour que la fiche Dolibarr affiche un montant
//!   juste. La vérité reste le calcul sur les échéances non réglées
//!   (`capital_restant`) : c'est lui qui fait foi, jamais `capital_position`.
//! - `llx_loan_schedule.amount_insurance` est écrit à 0. Dolibarr ne définit
//!   aucune répartition de `llx_loan.insurance_amount` sur les échéances (le
//!   montant d'assurance vit sur l'en-tête). Le répartir au prorata serait une
//!   invention qui fausserait la colonne du PDF.
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

use crate::reglement::{fin_de_mois, lire_date};

/// Longueur de `llx_loan.label` (`varchar(80)`), en caractères.
const LABEL_MAX: usize = 80;

/// Un taux annuel en pourcentage se divise par 12 puis par 100 pour donner le
/// taux mensuel : `3.5` % devient `0.0029166…`.
const MOIS_PAR_AN: i64 = 12;
const POURCENT: i64 = 100;

/// Une échéance calculée, sans identité en base : c'est ce que rend
/// `echeancier()` avant écriture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Echeance {
    /// Rang de l'échéance, à partir de 1.
    pub numero: u32,
    /// Date d'exigibilité, `AAAA-MM-JJ`.
    pub date: String,
    /// Part de capital remboursée par cette échéance.
    pub capital: Decimal,
    /// Part d'intérêts de cette échéance.
    pub interets: Decimal,
    /// Somme effectivement due : `capital + interets`. La dernière échéance
    /// diffère de quelques centimes, c'est le reliquat (voir la doc du module).
    pub mensualite: Decimal,
}

/// Une échéance telle qu'elle est stockée dans `llx_loan_schedule`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcheanceEnBase {
    pub rowid: i64,
    /// Rang reconstitué à la lecture (l'ordre des dates), pas une colonne.
    pub numero: u32,
    /// Date d'exigibilité (`datep`), `AAAA-MM-JJ`.
    pub datep: String,
    pub capital: Decimal,
    pub interets: Decimal,
    /// Montant d'assurance de la ligne : toujours 0, voir la doc du module.
    pub assurance: Decimal,
    pub mensualite: Decimal,
    /// Vrai quand `fk_payment_loan` est renseigné.
    pub regle: bool,
    pub fk_payment_loan: Option<i64>,
    pub num_payment: Option<String>,
}

/// Un prêt tel qu'affiché en liste : l'en-tête plus deux valeurs calculées.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pret {
    pub rowid: i64,
    pub label: String,
    /// Capital emprunté.
    pub capital: Decimal,
    /// Taux annuel en pourcentage (`3.5` = 3,5 %).
    pub taux: Decimal,
    pub nbterm: u32,
    /// Somme des parts de capital des échéances non réglées.
    pub capital_restant_du: Decimal,
    /// Date de la prochaine échéance non réglée, si le prêt n'est pas soldé.
    pub prochaine_echeance: Option<String>,
    pub fk_bank: Option<i64>,
    pub datestart: Option<String>,
    pub dateend: Option<String>,
    pub insurance_amount: Decimal,
    pub fk_projet: Option<i64>,
    pub active: bool,
}

/// Ce qu'il faut pour créer un prêt. Les champs facultatifs à `None` restent
/// `NULL` en base (jamais `0`, jamais `0000-00-00`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewPret {
    /// Libellé de la colonne `label`, obligatoire, 80 caractères au plus.
    pub label: String,
    /// Compte bancaire de l'opération. Doit exister dans l'entité s'il est fourni.
    pub fk_bank: Option<i64>,
    /// Capital emprunté, strictement positif.
    pub capital: Decimal,
    /// Montant d'assurance porté sur l'en-tête du prêt.
    pub insurance_amount: Decimal,
    /// Date de signature / de départ, `AAAA-MM-JJ`.
    pub datestart: String,
    /// Fin contractuelle, facultative.
    pub dateend: Option<String>,
    /// Nombre d'échéances, au moins 1.
    pub nbterm: u32,
    /// Taux annuel en pourcentage (`3.5` = 3,5 %), jamais négatif.
    pub rate: Decimal,
    pub fk_projet: Option<i64>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
}

// ---------------------------------------------------------------------------
// Calcul pur de l'échéancier
// ---------------------------------------------------------------------------

/// Échéancier d'un prêt à mensualités constantes.
///
/// Rend `nb_echeances` lignes numérotées de 1 à n, datées **de mois en mois à
/// partir du lendemain de `date_debut`** : on emprunte le jour de la signature,
/// la première mensualité tombe un mois plus tard (c'est aussi ce que fait
/// Dolibarr). La somme des `capital` égale `capital` au centime, le reliquat
/// étant porté par la dernière échéance — c'est la raison d'être de cette
/// fonction, voir la doc du module.
///
/// Le taux est annuel en pourcentage (`3.5` = 3,5 %) ; le taux mensuel vaut
/// `taux / 12 / 100`. Un taux nul est licite : la mensualité vaut alors
/// `capital / n`, sans intérêts.
///
/// La fonction ne panique jamais : elle rend un vecteur **vide** si
/// `nb_echeances` vaut 0 (un échéancier de zéro échéance est vide, ce n'est pas
/// une erreur de calcul), si le capital n'est pas strictement positif, si le
/// taux est négatif, ou si `date_debut` n'est pas une date `AAAA-MM-JJ`. Le
/// refus opposable à l'utilisateur est prononcé par `create()`, qui valide ses
/// entrées avant d'appeler ici.
pub fn echeancier(
    capital: Decimal,
    taux_annuel: Decimal,
    nb_echeances: u32,
    date_debut: &str,
) -> Vec<Echeance> {
    if nb_echeances == 0 || capital <= Decimal::ZERO || taux_annuel < Decimal::ZERO {
        return Vec::new();
    }
    let Some(ancre) = lire_date(date_debut) else {
        return Vec::new();
    };

    // Un nombre d'échéances non borné n'alloue pas « un peu trop » : la boucle
    // qui suit tourne 4 294 967 295 fois (fil bloqué plusieurs minutes) puis
    // `Vec::with_capacity` demande des centaines de gigaoctets, échoue, et le
    // processus est abandonné. Cinquante ans de mensualités couvrent tout prêt
    // réel ; `create` refuse la même borne avec un message.
    let nb_echeances = nb_echeances.min(600);
    let taux_mensuel = taux_annuel / Decimal::from(MOIS_PAR_AN) / Decimal::from(POURCENT);
    let n = Decimal::from(i64::from(nb_echeances));

    // Mensualité constante : m = C·i / (1 − (1+i)^−n). `powd` exigerait la
    // fonctionnalité « maths » de rust_decimal, qui n'est pas activée dans ce
    // dépôt : on élève (1+i) à la puissance n par multiplications successives,
    // ce qui reste exact (Decimal arrondit au plus à 28 décimales) et évite une
    // dépendance de plus.
    let mut mensualite = if taux_mensuel.is_zero() {
        capital / n
    } else {
        let mut facteur = Decimal::ONE;
        for _ in 0..nb_echeances {
            facteur *= Decimal::ONE + taux_mensuel;
        }
        capital * taux_mensuel / (Decimal::ONE - Decimal::ONE / facteur)
    };
    mensualite = round_money(mensualite);

    let mut restant = capital;
    let mut echeances = Vec::with_capacity(nb_echeances as usize);
    for numero in 1..=nb_echeances {
        // Les intérêts portent sur le capital encore dû *avant* l'échéance.
        let interets = round_money(restant * taux_mensuel);
        let capital_part = if numero == nb_echeances {
            // Dernière échéance : tout le reliquat, quelle que soit la
            // mensualité calculée. C'est ce qui fait tomber la somme pile.
            restant
        } else {
            let candidate = mensualite - interets;
            // Garde-fou : une mensualité arrondie ne peut pas dépasser le
            // capital restant ni devenir négative. Si cela arrivait (capital
            // minuscule, taux énorme), on solde le capital d'un coup et les
            // échéances suivantes ne portent que des intérêts nuls.
            if candidate <= Decimal::ZERO || candidate > restant {
                restant
            } else {
                candidate
            }
        };
        restant -= capital_part;
        echeances.push(Echeance {
            numero,
            date: date_echeance(ancre, numero),
            capital: capital_part,
            interets,
            mensualite: capital_part + interets,
        });
    }
    echeances
}

/// Date de la `mois_apres`-ième mensualité après `ancre`.
///
/// Le jour est ancré sur celui du départ et ramené au dernier jour du mois
/// cible quand il n'existe pas : partir du 31 janvier donne le 28 février, puis
/// le **31 mars** — pas le 28 mars. Sans cette ancre, un prêt signé un 29, 30
/// ou 31 décrocherait définitivement sur un jour plus court.
fn date_echeance(ancre: (i64, u32, u32), mois_apres: u32) -> String {
    let total = i64::from(ancre.1) - 1 + i64::from(mois_apres);
    let annee = ancre.0 + total / 12;
    let mois = (total % 12) as u32 + 1;
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
/// Le capital restant dû et la prochaine échéance sont deux sous-requêtes
/// corrélées : une jointure avec `GROUP BY` obligerait à recalculer le prêt
/// entier pour chaque échéance et à filtrer les non réglées dans un `HAVING`.
const COLS: &str = "l.rowid, l.label, CAST(l.capital AS CHAR), CAST(l.rate AS CHAR), \
     CAST(COALESCE(l.nbterm, 0) AS SIGNED), l.fk_bank, \
     DATE_FORMAT(l.datestart, '%Y-%m-%d'), DATE_FORMAT(l.dateend, '%Y-%m-%d'), \
     CAST(COALESCE(l.insurance_amount, 0) AS CHAR), l.fk_projet, l.active, \
     CAST(COALESCE((SELECT SUM(s.amount_capital) FROM llx_loan_schedule s \
        WHERE s.fk_loan = l.rowid AND s.fk_payment_loan IS NULL), 0) AS CHAR), \
     (SELECT DATE_FORMAT(MIN(s.datep), '%Y-%m-%d') FROM llx_loan_schedule s \
        WHERE s.fk_loan = l.rowid AND s.fk_payment_loan IS NULL)";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_pret(row: &Row) -> Pret {
    Pret {
        rowid: opt_i(row, 0).unwrap_or(0),
        label: opt_s(row, 1).unwrap_or_default(),
        capital: opt_dec(row, 2).unwrap_or_default(),
        taux: opt_dec(row, 3).unwrap_or_default(),
        // `nbterm` est un `real` : la conversion passe par un entier signé lu en
        // texte, jamais par un f64 intermédiaire.
        nbterm: opt_i(row, 4).unwrap_or(0) as u32,
        fk_bank: opt_i(row, 5),
        datestart: opt_s(row, 6),
        dateend: opt_s(row, 7),
        insurance_amount: opt_dec(row, 8).unwrap_or_default(),
        fk_projet: opt_i(row, 9),
        active: opt_i(row, 10).unwrap_or(1) != 0,
        capital_restant_du: round_money(opt_dec(row, 11).unwrap_or_default()),
        prochaine_echeance: opt_s(row, 12),
    }
}

/// Prêts de l'entité, du plus récent au plus ancien.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Pret>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {COLS} FROM llx_loan l \
         WHERE l.entity = :entity \
         ORDER BY COALESCE(l.datestart, DATE(l.datec)) DESC, l.rowid DESC \
         LIMIT :limite OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_pret).collect())
}

/// Un prêt précis, ou `None` s'il n'appartient pas à l'entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Pret>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql =
        format!("SELECT {COLS} FROM llx_loan l WHERE l.entity = :entity AND l.rowid = :rowid");
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_pret))
}

/// Échéances d'un prêt, dans l'ordre des dates.
///
/// La jointure sur `llx_loan` n'est pas décorative : `llx_loan_schedule` n'a pas
/// d'`entity`, seul le prêt en porte une.
pub fn echeances(
    pool: &Pool,
    entity: i64,
    fk_loan: i64,
) -> Result<Vec<EcheanceEnBase>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let rows: Vec<Row> = conn.exec(
        "SELECT s.rowid, DATE_FORMAT(s.datep, '%Y-%m-%d'), CAST(s.amount_capital AS CHAR), \
                CAST(s.amount_interest AS CHAR), CAST(COALESCE(s.amount_insurance, 0) AS CHAR), \
                s.fk_payment_loan, s.num_payment \
         FROM llx_loan_schedule s \
         INNER JOIN llx_loan l ON l.rowid = s.fk_loan \
         WHERE l.entity = :entity AND s.fk_loan = :loan \
         ORDER BY s.datep ASC, s.rowid ASC",
        params! { "entity" => entity, "loan" => fk_loan },
    )?;
    Ok(rows
        .iter()
        .enumerate()
        .map(|(rang, row)| {
            let capital = opt_dec(row, 2).unwrap_or_default();
            let interets = opt_dec(row, 3).unwrap_or_default();
            let assurance = opt_dec(row, 4).unwrap_or_default();
            let fk_payment_loan = opt_i(row, 5);
            EcheanceEnBase {
                // Le rang n'est pas stocké : il vient de l'ordre des dates.
                numero: rang as u32 + 1,
                rowid: opt_i(row, 0).unwrap_or(0),
                datep: opt_s(row, 1).unwrap_or_default(),
                capital,
                interets,
                assurance,
                mensualite: capital + interets + assurance,
                regle: fk_payment_loan.is_some(),
                fk_payment_loan,
                num_payment: opt_s(row, 6),
            }
        })
        .collect())
}

/// Somme des parts de capital non réglées : le capital restant dû.
///
/// C'est la seule mesure qui fait foi. `llx_loan.capital_position` n'en est
/// qu'un reflet, tenu à jour par `regler()`.
pub fn capital_restant(pool: &Pool, entity: i64, fk_loan: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    if !pret_existe(&mut conn, entity, fk_loan)? {
        return Err(format!("le prêt {fk_loan} n'existe pas dans l'entité {entity}").into());
    }
    let reste = reste_a_payer(&mut conn, fk_loan)?;
    Ok(round_money(reste))
}

// ---------------------------------------------------------------------------
// Écriture
// ---------------------------------------------------------------------------

/// Crée un prêt et son échéancier, dans la même transaction.
///
/// Refuse : un libellé vide ou trop long, un capital nul ou négatif, un taux
/// négatif, une assurance négative, `nbterm = 0`, une date illisible, une fin
/// antérieure au départ, et un compte bancaire absent de l'entité. Tous ces
/// refus sont prononcés **avant** toute écriture : une seule ligne fausse
/// suffirait à déséquilibrer l'échéancier pour des années.
pub fn create(pool: &Pool, entity: i64, new: &NewPret) -> Result<i64, Box<dyn Error>> {
    let label = new.label.trim();
    if label.is_empty() {
        return Err("le libellé du prêt est obligatoire".into());
    }
    if label.chars().count() > LABEL_MAX {
        return Err(
            format!("le libellé du prêt dépasse {LABEL_MAX} caractères (colonne label)").into(),
        );
    }
    let capital = round_money(new.capital);
    if capital <= Decimal::ZERO {
        return Err("le capital du prêt doit être strictement positif".into());
    }
    if new.rate < Decimal::ZERO {
        return Err("le taux du prêt ne peut pas être négatif".into());
    }
    let assurance = round_money(new.insurance_amount);
    if assurance < Decimal::ZERO {
        return Err("le montant d'assurance du prêt ne peut pas être négatif".into());
    }
    if new.nbterm == 0 {
        return Err("le nombre d'échéances du prêt doit être au moins 1".into());
    }
    // Refus explicite plutôt que correction silencieuse : l'utilisateur doit
    // savoir que sa saisie n'a pas été prise telle quelle. Même borne que
    // `echeancier`, qui la protège aussi quand on l'appelle directement.
    if new.nbterm > 600 {
        return Err(format!(
            "le nombre d'échéances ({}) dépasse 600 : un prêt de plus de cinquante \
             ans n'a pas de sens, et le calcul refuserait de l'allouer",
            new.nbterm
        )
        .into());
    }
    let datestart = new.datestart.trim();
    if lire_date(datestart).is_none() {
        return Err(format!(
            "la date de départ du prêt « {} » n'est pas une date AAAA-MM-JJ valide",
            new.datestart
        )
        .into());
    }
    // Une date de fin vide est un champ non renseigné, pas une date invalide :
    // les formulaires HTML renvoient toujours une chaîne.
    let dateend = new
        .dateend
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty());
    if let Some(fin) = dateend {
        if lire_date(fin).is_none() {
            return Err(format!(
                "la date de fin du prêt « {fin} » n'est pas une date AAAA-MM-JJ valide"
            )
            .into());
        }
        // Comparaison de chaînes : sur des dates ISO, l'ordre lexicographique
        // est l'ordre chronologique.
        if fin <= datestart {
            return Err("la date de fin du prêt doit être postérieure à son départ".into());
        }
    }

    let echeances = echeancier(capital, new.rate, new.nbterm, datestart);
    if echeances.is_empty() {
        return Err("échéancier vide : vérifiez le capital, le taux et la date de départ".into());
    }

    let mut conn = pool.get_conn()?;
    if let Some(banque) = new.fk_bank {
        let existe = compte_existe(&mut conn, entity, banque)?;
        if !existe {
            return Err(
                format!("le compte bancaire {banque} n'existe pas dans l'entité {entity}").into(),
            );
        }
    }
    let banque_echeance = new.fk_bank.unwrap_or(0);

    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        // `nbterm` est un `real`, `capital_position` un `double(24,8)`. On écrit
        // `capital_position = capital` : aucun règlement n'a encore eu lieu.
        // `fk_user_author` reste NULL (clé vers `llx_user`, aucune session
        // ouverte) et `active` vaut 1 (colonne NOT NULL).
        "INSERT INTO llx_loan \
         (entity, datec, label, fk_bank, capital, insurance_amount, datestart, dateend, \
          nbterm, rate, note_private, note_public, capital_position, date_position, paid, \
          fk_projet, fk_user_author, active) \
         VALUES \
         (:entity, NOW(), :label, :banque, :capital, :assurance, :datestart, :dateend, \
          :nbterm, :taux, :note_private, :note_public, :capital_position, :date_position, 0, \
          :projet, NULL, 1)",
        params! {
            "entity" => entity,
            "label" => label,
            "banque" => new.fk_bank,
            "capital" => capital.to_string(),
            "assurance" => assurance.to_string(),
            "datestart" => datestart,
            "dateend" => dateend,
            "nbterm" => new.nbterm,
            "taux" => new.rate.to_string(),
            "note_private" => new.note_private.as_deref(),
            "note_public" => new.note_public.as_deref(),
            "capital_position" => capital.to_string(),
            // Même valeur que `datestart`, nom distinct : un emplacement nommé
            // réutilisé deux fois dans la même requête n'est pas garanti par le
            // pilote, deux noms le sont.
            "date_position" => datestart,
            "projet" => new.fk_projet,
        },
    )?;
    let rowid = tx.last_insert_id().unwrap_or(0) as i64;

    for echeance in &echeances {
        tx.exec_drop(
            // `fk_typepayment` ET `fk_bank` sont NOT NULL : un moyen de paiement
            // n'a pas de sens sur une échéance prévue (0 = aucun), le compte est
            // celui du prêt (0 s'il n'y en a pas).
            "INSERT INTO llx_loan_schedule \
             (fk_loan, datec, datep, amount_capital, amount_insurance, amount_interest, \
              fk_typepayment, num_payment, note_private, note_public, fk_bank, \
              fk_payment_loan, fk_user_creat) \
             VALUES \
             (:loan, NOW(), :datep, :capital, 0, :interets, \
              0, NULL, NULL, NULL, :banque, NULL, NULL)",
            params! {
                "loan" => rowid,
                "datep" => echeance.date.as_str(),
                "capital" => echeance.capital.to_string(),
                "interets" => echeance.interets.to_string(),
                "banque" => banque_echeance,
            },
        )?;
    }
    tx.commit()?;
    Ok(rowid)
}

/// Marque une échéance comme réglée et rend l'identifiant du paiement créé.
///
/// Le règlement n'existe pas « sur » l'échéance dans Dolibarr : il vit dans
/// `llx_payment_loan`, et l'échéance le référence par `fk_payment_loan`. On
/// recopie donc le montant de l'échéance dans un paiement daté du jour, puis on
/// pointe l'échéance dessus.
///
/// Le refus d'un double règlement ne repose pas sur la lecture préalable (qui
/// ne sert qu'au message) mais sur l'`UPDATE` conditionnel
/// `... AND fk_payment_loan IS NULL` : deux clics simultanés ne peuvent pas
/// passer tous les deux, le second ne touche aucune ligne et la transaction est
/// annulée.
pub fn regler(
    pool: &Pool,
    entity: i64,
    fk_loan: i64,
    fk_echeance: i64,
    fk_bank: i64,
) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let Some(banque_pret) = banque_du_pret(&mut conn, entity, fk_loan)? else {
        return Err(format!("le prêt {fk_loan} n'existe pas dans l'entité {entity}").into());
    };

    // Le compte fourni doit exister, comme pour la création : écrire un
    // identifiant fantôme dans `fk_bank` rendrait le rapprochement impossible.
    let banque = if fk_bank > 0 {
        let existe = compte_existe(&mut conn, entity, fk_bank)?;
        if !existe {
            return Err(format!(
                "le compte bancaire {fk_bank} n'existe pas dans l'entité {entity}"
            )
            .into());
        }
        fk_bank
    } else {
        banque_pret.unwrap_or(0)
    };

    let ligne: Option<Row> = conn.exec_first(
        "SELECT s.rowid, CAST(s.amount_capital AS CHAR), CAST(s.amount_interest AS CHAR), \
                CAST(COALESCE(s.amount_insurance, 0) AS CHAR), s.fk_payment_loan, \
                s.fk_typepayment, s.num_payment, s.fk_bank \
         FROM llx_loan_schedule s \
         INNER JOIN llx_loan l ON l.rowid = s.fk_loan \
         WHERE l.entity = :entity AND s.fk_loan = :loan AND s.rowid = :echeance",
        params! { "entity" => entity, "loan" => fk_loan, "echeance" => fk_echeance },
    )?;
    let Some(ligne) = ligne else {
        return Err(format!(
            "l'échéance {fk_echeance} n'appartient pas au prêt {fk_loan} de l'entité {entity}"
        )
        .into());
    };
    if let Some(paiement) = opt_i(&ligne, 4) {
        return Err(format!(
            "l'échéance {fk_echeance} du prêt {fk_loan} est déjà réglée (paiement {paiement})"
        )
        .into());
    }

    let capital = opt_dec(&ligne, 1).unwrap_or_default();
    let interets = opt_dec(&ligne, 2).unwrap_or_default();
    let assurance = opt_dec(&ligne, 3).unwrap_or_default();
    let typepaiement = opt_i(&ligne, 5).unwrap_or(0);
    let num_payment = opt_s(&ligne, 6);

    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        "INSERT INTO llx_payment_loan \
         (fk_loan, datec, datep, amount_capital, amount_insurance, amount_interest, \
          fk_typepayment, num_payment, note_private, note_public, fk_bank, fk_user_creat) \
         VALUES \
         (:loan, NOW(), CURDATE(), :capital, :assurance, :interets, \
          :typepaiement, :num, NULL, NULL, :banque, NULL)",
        params! {
            "loan" => fk_loan,
            "capital" => capital.to_string(),
            "assurance" => assurance.to_string(),
            "interets" => interets.to_string(),
            "typepaiement" => typepaiement,
            "num" => num_payment.as_deref(),
            "banque" => banque,
        },
    )?;
    let paiement = tx.last_insert_id().unwrap_or(0) as i64;

    tx.exec_drop(
        "UPDATE llx_loan_schedule SET fk_payment_loan = :paiement \
         WHERE rowid = :echeance AND fk_loan = :loan AND fk_payment_loan IS NULL",
        params! { "paiement" => paiement, "echeance" => fk_echeance, "loan" => fk_loan },
    )?;
    if tx.affected_rows() == 0 {
        tx.rollback()?;
        return Err(format!("l'échéance {fk_echeance} du prêt {fk_loan} est déjà réglée").into());
    }

    // Reflet pour la fiche Dolibarr. `paid` ne passe à 1 que quand plus rien
    // n'est dû, sinon la fiche annoncerait un prêt soldé à mi-parcours.
    let reste = reste_a_payer(&mut tx, fk_loan)?;
    let solde = if reste <= Decimal::ZERO { 1i16 } else { 0i16 };
    tx.exec_drop(
        "UPDATE llx_loan SET capital_position = :reste, date_position = CURDATE(), \
                paid = :paid, tms = NOW() \
         WHERE entity = :entity AND rowid = :loan",
        params! {
            "reste" => round_money(reste).to_string(),
            "paid" => solde,
            "entity" => entity,
            "loan" => fk_loan,
        },
    )?;
    tx.commit()?;
    Ok(paiement)
}

// ---------------------------------------------------------------------------
// Petites lectures partagées
// ---------------------------------------------------------------------------

fn compte_existe<Q>(conn: &mut Q, entity: i64, rowid: i64) -> Result<bool, Box<dyn Error>>
where
    Q: Queryable,
{
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_bank_account WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    Ok(existe.is_some())
}

fn pret_existe<Q>(conn: &mut Q, entity: i64, rowid: i64) -> Result<bool, Box<dyn Error>>
where
    Q: Queryable,
{
    let existe: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_loan WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    Ok(existe.is_some())
}

/// `fk_bank` du prêt : `None` si le prêt n'existe pas dans l'entité, sinon la
/// valeur de la colonne (elle-même éventuellement `NULL`).
fn banque_du_pret<Q>(
    conn: &mut Q,
    entity: i64,
    fk_loan: i64,
) -> Result<Option<Option<i64>>, Box<dyn Error>>
where
    Q: Queryable,
{
    let banque: Option<Option<i64>> = conn.exec_first(
        "SELECT fk_bank FROM llx_loan WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_loan },
    )?;
    Ok(banque)
}

/// Somme des parts de capital non réglées, brute (sans arrondi d'affichage).
fn reste_a_payer<Q>(conn: &mut Q, fk_loan: i64) -> Result<Decimal, Box<dyn Error>>
where
    Q: Queryable,
{
    let somme: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(amount_capital), 0) AS CHAR) \
         FROM llx_loan_schedule \
         WHERE fk_loan = :loan AND fk_payment_loan IS NULL",
        params! { "loan" => fk_loan },
    )?;
    Ok(somme
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default())
}
