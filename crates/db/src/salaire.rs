//! Salaires (`llx_salary`) et leurs versements (`llx_payment_salary`).
//!
//! Un salaire, dans Dolibarr, n'est **pas** un bulletin de paie calculé : c'est
//! un fait de paie enregistré. Le noyau ne connaît ni le brut, ni les
//! cotisations, ni le net imposable — il ouvre une fiche avec un montant, une
//! date de paiement, un salarié, et il attend qu'on lui dise « c'est versé ».
//! Le cabinet de paie produit le bulletin ; le logiciel de gestion retient
//! seulement combien et quand. Ce module tient ce rôle, rien de plus : il
//! n'invente aucun taux, aucun plafond, aucune assiette.
//!
//! Les montants sont des `double(24,8)` en base. Ils sortent en
//! `CAST(... AS CHAR)` puis passent par `Decimal::from_str` : jamais par un
//! flottant, sans quoi le total annuel et la somme des lignes divergent au
//! centime à la première addition.
//!
//! # Numérotation : il n'y en a pas, et c'est le `rowid` qui sert de référence
//!
//! C'est le point qu'on cherche en premier et qui n'existe pas. Vérifié dans le
//! noyau (`_vendor/dolibarr-ref/htdocs/salaries/class/salary.class.php`) :
//!
//! - `create()` n'écrit **jamais** `ref` : la colonne n'apparaît pas dans
//!   l'`INSERT` (contrairement à un devis ou une facture, qui appellent un
//!   module de numérotation) ;
//! - `fetch()` fait `$this->ref = $obj->rowid;` — la référence affichée **est**
//!   l'identifiant technique ;
//! - `htdocs/core/modules/salaries/` ne contient qu'un modèle PDF
//!   (`doc/pdf_standard_salary.modules.php`) et sa classe de base : aucun
//!   `mod_salary_*` numérotant, donc aucun `MODULE_SALARY_ADDON` à lire ;
//! - `list.php` cherche un salaire par `AND s.rowid = <ref>` quand on saisit
//!   une référence dans le filtre : la « référence » d'un salaire est un
//!   entier, pas une chaîne `SA2611-0001`.
//!
//! La convention est donc : **la référence d'un salaire est son `rowid`**. Le
//! noyau laisse la colonne à `NULL` et la reconstitue à la lecture ; nous
//! l'écrivons en base juste après l'insertion, dans la même transaction. Deux
//! raisons, aucune n'étant esthétique :
//!
//! 1. la liste générique de l'application (`crates/web/src/pages/listes_ecran.rs`,
//!    entrée `Salaries`) lit `s.ref` : avec `NULL`, la colonne « Réf. » sort
//!    vide et l'écran paraît cassé ;
//! 2. l'index unique `uk_salary_ref (ref, entity)` existe déjà : y écrire le
//!    `rowid` rend le doublon **impossible par construction**, puisqu'un
//!    `rowid` ne se répète pas. Aucun compteur à verrouiller, aucune course à
//!    perdre entre deux créations simultanées.
//!
//! # Dates : `datep` est la date de paiement, et le noyau ne l'écrit pas
//!
//! `llx_salary` porte quatre dates : `datep` (paiement), `datev` (valeur),
//! `datesp` / `dateep` (période de paie). Curiosité du noyau : `create()` et
//! `update()` **commentent** l'écriture de `datep` et `datev` (les lignes sont
//! précédées de `//`), alors que l'écran les demande (`salaries/card.php`,
//! lignes 80-81) et que la liste trie dessus par défaut
//! (`$sortfield = "s.datep,s.rowid"`). Résultat : une base Dolibarr d'origine
//! range tous ses salaires dans le même seau de dates vides.
//!
//! Nous les écrivons, parce que les colonnes existent et que le tri de la liste
//! ne veut rien dire sans elles. En revanche `datesp` / `dateep` restent à
//! `NULL` : `NouveauSalaire` ne porte pas la période de paie, et la déduire de
//! la date de paiement serait une invention — un bulletin de décembre se paie
//! souvent en janvier.
//!
//! # Comptabilité : aucune écriture, ni bancaire ni générale
//!
//! **Le versement d'un salaire n'écrit ni ligne `llx_bank`, ni écriture dans
//! `llx_accounting_bookkeeping`.** Ce n'est pas un oubli, c'est une décision,
//! et voici les trois raisons.
//!
//! 1. **Il n'existe aucun équivalent salaire côté écritures.** `ecritures.rs`
//!    expose `facture_vente`, `facture_achat`, `reglement_client` et
//!    `a_nouveaux`, avec cinq comptes only : `411000` clients, `401000`
//!    fournisseurs, `706000` ventes, `601000` achats, `512000` banque, plus les
//!    deux comptes de TVA. `persist_ecritures::ecrire_reglement_client` n'a pas
//!    de jumeau pour la paie. Écrire une contrepartie de salaire demanderait un
//!    compte de charge de personnel (`641…`) et un compte de dettes sociales
//!    (`421…`, `43…`) : **aucun des deux n'existe dans la crate, et je n'en
//!    invente pas**. Un compte faux ne se voir pas à la saisie, il se voit à la
//!    balance trois mois plus tard.
//! 2. **Le compte dépend de la société, pas du logiciel.** Le plan comptable
//!    est une donnée du client (`llx_accounting_account`), pas une constante du
//!    code. Ici la table est même vide : rien ne garantit qu'un `421000` ouvert
//!    dans nos tests corresponde à celui du cabinet. Le portage laisse donc la
//!    comptabilité générale au noyau et aux écrans dédiés.
//! 3. **Le salaire n'est pas un mouvement de trésorerie mais une charge de
//!    personnel** ; la banque n'en voit que le virement. Confondre les deux
//!    ferait entrer la charge au jour du débit et non à la période travaillée.
//!
//! Même raisonnement pour `llx_bank`, avec le précédent déjà écrit dans la crate
//! ([`crate::achat_paiement`]) : on ne devine pas un compte pour « faire vivant ».
//! Le rapprochement bancaire est un acte distinct, qui choisit un compte et une
//! ligne ; il n'a rien à faire dans la constatation d'un versement de salaire.
//! Conséquence assumée et visible : `llx_salary.fk_bank` et
//! `llx_payment_salary.fk_bank` désignent, dans le noyau, une **ligne** de
//! `llx_bank` — nous n'en créons pas, donc `llx_salary.fk_bank` reste `NULL`, et
//! la colonne `fk_bank` du versement reçoit l'identifiant du **compte**
//! (`llx_bank_account`), comme le fait déjà [`crate::dons`] : c'est le seul
//! endroit où le compte choisi par l'utilisateur peut atterrir sans mentir sur
//! une ligne bancaire qui n'existe pas.
//!
//! # Un salaire naît non payé
//!
//! `card.php` force `$object->paye = 0` avant l'appel à `create()` (ligne 407),
//! et l'`INSERT` du noyau n'inclut même pas la colonne. Le champ `paye` de
//! [`NouveauSalaire`] existe donc dans la signature, mais [`creer`] **refuse**
//! la valeur `true` : un salaire marqué payé sans ligne dans
//! `llx_payment_salary` est un salaire dont personne ne peut plus dire quand
//! l'argent est sorti, et que [`payer`] refusera ensuite de régulariser. Le
//! paiement se constate, il ne se déclare pas.

use std::error::Error;
use std::str::FromStr;

use money::round_money;
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// `llx_salary.paye` : le salaire est enregistré, le versement reste à faire.
///
/// C'est la constante `Salary::STATUS_UNPAID` du noyau.
pub const STATUT_NON_PAYE: i16 = 0;

/// `llx_salary.paye` : le versement est constaté.
///
/// C'est la constante `Salary::STATUS_PAID` du noyau. Il n'y a pas d'autre
/// état : Dolibarr ne connaît ni « en préparation » ni « en litige ».
pub const STATUT_PAYE: i16 = 1;

/// Un salaire tel qu'il est stocké et relu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Salaire {
    pub rowid: i64,
    pub entity: i64,
    /// Référence affichable : le `rowid` en texte (cf. la doc du module).
    pub ref_: String,
    /// Salarié concerné (`llx_salary.fk_user`).
    pub fk_user: i64,
    /// Login du salarié (`llx_user.login`), pour ne pas afficher un numéro.
    pub login: Option<String>,
    /// Date de paiement (`AAAA-MM-JJ`), telle que déclarée.
    pub datep: Option<String>,
    /// Date de valeur.
    pub datev: Option<String>,
    /// Date de création de la fiche.
    pub datec: Option<String>,
    /// Début de la période de paie, si le noyau l'a renseignée.
    pub periode_debut: Option<String>,
    /// Fin de la période de paie.
    pub periode_fin: Option<String>,
    /// Montant net à verser.
    pub amount: Decimal,
    /// Salaire de référence (`llx_salary.salary`), facultatif : le noyau le
    /// garde pour distinguer le montant versé du salaire contractuel.
    pub salaire_reference: Option<Decimal>,
    pub label: Option<String>,
    /// Projet analytique imputé, s'il y en a un.
    pub fk_project: Option<i64>,
    /// Moyen de paiement (`llx_c_paiement.id`, `0` tant qu'il n'est pas choisi).
    pub fk_typepayment: i64,
    /// Code du moyen de paiement (`VIR`, `CHQ`…), lu au dictionnaire.
    pub type_payment_code: Option<String>,
    /// Numéro du chèque ou du virement.
    pub num_payment: Option<String>,
    /// Ligne bancaire du noyau : toujours `NULL` ici (cf. doc du module).
    pub fk_bank: Option<i64>,
    /// Compte bancaire retenu au versement (`llx_bank_account.rowid`).
    pub fk_account: Option<i64>,
    /// `false` tant que le versement n'est pas constaté.
    pub paye: bool,
}

/// Ce qu'il faut pour ouvrir une fiche de paie.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NouveauSalaire {
    /// Salarié, obligatoire : un salaire sans personne n'est pas un salaire.
    pub fk_user: i64,
    /// Date de paiement (`AAAA-MM-JJ`).
    pub datep: String,
    /// Date de valeur (`AAAA-MM-JJ`) ; elle ne peut pas précéder `datep`.
    pub datev: String,
    /// Montant net à verser, toujours positif.
    pub amount: Decimal,
    /// Libellé ; vide interdit, c'est la seule phrase qui explique le montant.
    pub label: String,
    /// Projet analytique facultatif (`llx_salary.fk_projet`).
    pub fk_project: Option<i64>,
    /// Refusé à `true` : un salaire naît non payé (cf. doc du module).
    pub paye: bool,
}

/// Colonnes lues, dans l'ordre attendu par [`map_row`].
const SELECT_COLS: &str = "s.rowid, s.entity, s.ref, s.fk_user, u.login, \
     DATE_FORMAT(s.datep, '%Y-%m-%d'), DATE_FORMAT(s.datev, '%Y-%m-%d'), \
     DATE_FORMAT(s.datec, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(s.datesp, '%Y-%m-%d'), DATE_FORMAT(s.dateep, '%Y-%m-%d'), \
     CAST(s.amount AS CHAR), CAST(s.salary AS CHAR), s.label, s.fk_projet, \
     s.fk_typepayment, cp.code, s.num_payment, s.fk_bank, s.fk_account, s.paye";

/// Jointures communes : le login du salarié et le code du moyen de paiement.
///
/// `llx_c_paiement` est un dictionnaire global : ses lignes ne portent pas
/// l'entité de la société, d'où l'absence de filtre d'entité sur cette
/// jointure. Un identifiant de moyen de paiement qui ne résout rien laisse
/// simplement `type_payment_code` à `NULL`.
const JOINTURES: &str = "LEFT JOIN llx_user u ON u.rowid = s.fk_user \
     LEFT JOIN llx_c_paiement cp ON cp.id = s.fk_typepayment";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// La référence d'un salaire est son `rowid` : on retombe dessus même si la
/// colonne est vide, ce qui est le cas de tout salaire créé par le noyau PHP.
fn reference(ref_colonne: Option<String>, rowid: i64) -> String {
    match ref_colonne {
        Some(r) if !r.trim().is_empty() => r.trim().to_string(),
        _ => rowid.to_string(),
    }
}

fn map_row(row: &Row) -> Salaire {
    let rowid = opt_i(row, 0).unwrap_or(0);
    Salaire {
        rowid,
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: reference(opt_s(row, 2), rowid),
        fk_user: opt_i(row, 3).unwrap_or(0),
        login: opt_s(row, 4),
        datep: opt_s(row, 5),
        datev: opt_s(row, 6),
        datec: opt_s(row, 7),
        periode_debut: opt_s(row, 8),
        periode_fin: opt_s(row, 9),
        amount: opt_dec(row, 10).unwrap_or_default(),
        salaire_reference: opt_dec(row, 11),
        label: opt_s(row, 12),
        fk_project: opt_i(row, 13),
        fk_typepayment: opt_i(row, 14).unwrap_or(0),
        type_payment_code: opt_s(row, 15),
        num_payment: opt_s(row, 16),
        fk_bank: opt_i(row, 17),
        fk_account: opt_i(row, 18),
        paye: opt_i(row, 19).unwrap_or(0) != 0,
    }
}

/// Salaires d'une entité, du plus récent au plus ancien, avec le login.
///
/// Le classement reprend celui de `salaries/list.php` (`datep DESC, rowid
/// DESC`), avec une nuance : quand `datep` est vide — ce que fait le noyau,
/// qui n'écrit jamais cette colonne — on classe sur `datec`, sinon les fiches
/// du noyau s'entasseraient toutes en fin de liste, dans un ordre arbitraire.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Salaire>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_salary s {JOINTURES} \
         WHERE s.entity = :entity \
         ORDER BY COALESCE(s.datep, DATE(s.datec)) DESC, s.rowid DESC \
         LIMIT :limite OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_row).collect())
}

/// Un salaire précis, ou `None` s'il n'existe pas dans cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Salaire>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_salary s {JOINTURES} \
         WHERE s.entity = :entity AND s.rowid = :rowid"
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_row))
}

/// Ouvre une fiche de paie et renvoie son `rowid` (qui est aussi sa référence).
///
/// Refuse, dans cet ordre :
///
/// - un libellé vide ;
/// - un salarié qui n'existe pas dans l'entité ;
/// - un montant négatif ;
/// - une date mal formée ou inexistante ;
/// - une date de valeur antérieure à la date de paiement ;
/// - `paye = true` (un salaire ne naît pas payé, cf. doc du module).
///
/// Les deux premières colonnes de date écrites sont `datep` et `datev` ; la
/// période de paie (`datesp`/`dateep`) reste vide, faute de la connaître.
/// Le moyen de paiement par defaut : le virement s'il existe, sinon le premier
/// actif, sinon le premier tout court.
///
/// La colonne est obligatoire et porte une cle etrangere : il faut **un
/// identifiant qui existe**. Renvoyer 0 ferait echouer la base (1452), ce qui
/// est pire qu'un defaut explicite — au moins celui-ci se corrige au versement.
fn moyen_par_defaut(tx: &mut mysql::Transaction<'_>) -> Result<i64, Box<dyn Error>> {
    let virement: Option<i64> = tx.exec_first(
        "SELECT id FROM llx_c_paiement WHERE code = 'VIR' ORDER BY id LIMIT 1",
        (),
    )?;
    if let Some(id) = virement {
        return Ok(id);
    }
    let actif: Option<i64> = tx.exec_first(
        "SELECT id FROM llx_c_paiement WHERE active = 1 ORDER BY position, id LIMIT 1",
        (),
    )?;
    let premier: Option<i64> =
        tx.exec_first("SELECT id FROM llx_c_paiement ORDER BY id LIMIT 1", ())?;
    premier
        .or(actif)
        .ok_or_else(|| "aucun moyen de paiement dans llx_c_paiement".into())
}

pub fn creer(pool: &Pool, entity: i64, new: &NouveauSalaire) -> Result<i64, Box<dyn Error>> {
    let libelle = new.label.trim();
    if libelle.is_empty() {
        return Err(
            "le libellé du salaire est obligatoire : c'est lui qui dit à \
                    quoi se rapporte le montant"
                .into(),
        );
    }
    if new.amount < Decimal::ZERO {
        return Err(format!(
            "le montant {} est négatif : un salaire se verse, il ne se reverse pas \
             (une régularisation est un second bulletin positif)",
            new.amount
        )
        .into());
    }
    if new.paye {
        return Err(
            "un salaire ne se crée pas déjà payé : utilisez `payer`, qui \
                    enregistre la date et le compte du versement dans \
                    llx_payment_salary"
                .into(),
        );
    }

    let debut = new.datep.trim();
    let valeur = new.datev.trim();
    analyser_date(debut)?;
    analyser_date(valeur)?;
    // Comparaison lexicographique : sur `AAAA-MM-JJ`, l'ordre alphabétique est
    // l'ordre chronologique. Aucune conversion, donc aucune erreur de fuseau.
    if valeur < debut {
        return Err(format!(
            "la date de valeur ({valeur}) précède la date de paiement ({debut}) : \
             un versement ne peut pas avoir de valeur avant d'exister"
        )
        .into());
    }

    // Le salarié est vérifié en base : `llx_salary.fk_user` n'a pas de clé
    // étrangère, un identifiant inventé passerait sans bruit et la fiche
    // n'apparaîtrait dans l'écran de personne.
    {
        let mut conn = pool.get_conn()?;
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_user WHERE entity = :entity AND rowid = :rowid",
            params! { "entity" => entity, "rowid" => new.fk_user },
        )?;
        if existe.is_none() {
            return Err(format!(
                "l'utilisateur {} n'existe pas dans l'entité {entity}",
                new.fk_user
            )
            .into());
        }
    }

    let mut conn = pool.get_conn()?;
    let mut tx = conn.start_transaction(TxOpts::default())?;

    // `fk_typepayment` est NOT NULL **et** porte une cle etrangere vers
    // `llx_c_paiement` : ecrire 0 est refuse par la base (erreur 1452). On met
    // le virement — le moyen le plus courant pour un salaire — et `payer` le
    // remplacera par le moyen reel du versement. Lu **avant** l'instruction :
    // emprunter `tx` deux fois ne compile pas, et c'est heureux — la seconde
    // lecture verrait un etat a moitie ecrit.
    let moyen = moyen_par_defaut(&mut tx)?;
    tx.exec_drop(
        // `fk_user_author` n'est pas écrite : aucune session n'est ouverte ici,
        // et recopier le salarié donnerait un auteur faux — il n'a pas créé sa
        // propre fiche. `fk_typepayment` reçoit le virement par défaut : la
        // colonne est NOT NULL et porte une clé étrangère, donc 0 est refusé par
        // la base. `payer` le remplacera par le moyen réel du versement.
        "INSERT INTO llx_salary \
         (entity, fk_user, datep, datev, amount, label, fk_projet, \
          fk_typepayment, paye, datec) \
         VALUES (:entity, :salarie, :datep, :datev, :montant, :libelle, :projet, \
          :moyen, :paye, NOW())",
        params! {
            "entity" => entity,
            "salarie" => new.fk_user,
            "datep" => debut,
            "datev" => valeur,
            "montant" => new.amount.to_string(),
            "libelle" => libelle,
            "projet" => new.fk_project,
            "moyen" => moyen,
            "paye" => STATUT_NON_PAYE,
        },
    )?;
    let rowid = tx.last_insert_id().unwrap_or(0) as i64;

    // La référence EST le rowid (cf. doc du module) : on l'écrit pour que la
    // liste ne montre pas une colonne vide, et l'index unique
    // `uk_salary_ref (ref, entity)` ne peut plus être pris en défaut.
    let ref_texte = rowid.to_string();
    tx.exec_drop(
        "UPDATE llx_salary SET ref = :reference WHERE rowid = :rowid",
        params! { "reference" => ref_texte.as_str(), "rowid" => rowid },
    )?;

    tx.commit()?;
    Ok(rowid)
}

/// Constate le versement d'un salaire et renvoie le `rowid` du paiement.
///
/// Écrit deux choses, dans une seule transaction :
///
/// 1. la trace du versement dans `llx_payment_salary` — montant **recopié** du
///    salaire (on ne le recalcule pas : ce qui est dû est ce qui est écrit sur
///    la fiche), date, moyen de paiement, numéro, compte ;
/// 2. le drapeau `llx_salary.paye = 1`, avec le moyen de paiement et le compte
///    sur la fiche, parce que c'est la fiche que lisent l'écran et les totaux.
///
/// `fk_bank` est l'identifiant du **compte bancaire** (`llx_bank_account`),
/// comme dans [`crate::dons`] : aucune ligne `llx_bank` n'est créée (cf. doc du
/// module), donc `llx_salary.fk_bank` — qui désigne une ligne bancaire dans le
/// noyau — reste vide, et `llx_payment_salary.fk_bank` reçoit le compte.
///
/// Refuse un salaire inconnu, déjà payé, de montant nul, un compte bancaire
/// inexistant, un moyen de paiement inconnu, et une date mal formée.
///
/// La date de paiement n'est **pas** recopiée dans `llx_salary.datep` : la
/// fiche garde la date déclarée, le versement porte la date réelle. Deux
/// colonnes, deux faits, aucune ambiguïté sur ce qu'une correction modifie.
pub fn payer(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    fk_bank: i64,
    fk_typepayment: i64,
    date: &str,
    num_payment: Option<&str>,
) -> Result<i64, Box<dyn Error>> {
    let jour = date.trim();
    analyser_date(jour)?;
    let numero = num_payment.map(str::trim).filter(|n| !n.is_empty());

    let mut conn = pool.get_conn()?;
    let mut tx = conn.start_transaction(TxOpts::default())?;

    // `FOR UPDATE` : sans lui, deux appels simultanés lisent tous deux
    // `paye = 0` et écrivent deux versements pour le même bulletin.
    let fiche: Option<(String, i64)> = tx.exec_first(
        "SELECT CAST(amount AS CHAR), paye FROM llx_salary \
         WHERE entity = :entity AND rowid = :rowid FOR UPDATE",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some((montant_texte, paye)) = fiche else {
        tx.rollback()?;
        return Err(format!("le salaire {rowid} n'existe pas dans l'entité {entity}").into());
    };
    if paye != i64::from(STATUT_NON_PAYE) {
        tx.rollback()?;
        return Err(format!("le salaire {rowid} est déjà payé").into());
    }
    let montant = Decimal::from_str(montant_texte.trim())?;
    if montant <= Decimal::ZERO {
        tx.rollback()?;
        return Err(format!(
            "le salaire {rowid} porte un montant nul ({montant}) : il n'y a rien à verser"
        )
        .into());
    }

    let compte: Option<i64> = tx.exec_first(
        "SELECT rowid FROM llx_bank_account WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => fk_bank },
    )?;
    if compte.is_none() {
        tx.rollback()?;
        return Err(
            format!("le compte bancaire {fk_bank} n'existe pas dans l'entité {entity}").into(),
        );
    }

    // Existence seule, sans filtre sur `active` : un moyen de paiement
    // désactivé reste un moyen réel, et un versement déjà décidé ne se bloque
    // pas pour cette raison (même règle que `dons.rs`). Pas de filtre d'entité
    // non plus : `llx_c_paiement` est un dictionnaire global.
    let moyen: Option<i64> = tx.exec_first(
        "SELECT id FROM llx_c_paiement WHERE id = :moyen",
        params! { "moyen" => fk_typepayment },
    )?;
    if moyen.is_none() {
        tx.rollback()?;
        return Err(format!(
            "le moyen de paiement {fk_typepayment} n'existe pas dans le dictionnaire \
             llx_c_paiement"
        )
        .into());
    }

    tx.exec_drop(
        // `fk_user_author` reste NULL : la signature de `payer` ne porte pas
        // d'auteur, et recopier le salarié désignerait le mauvais homme.
        // `datev` prend la date du versement, faute d'une seconde date.
        "INSERT INTO llx_payment_salary \
         (entity, fk_salary, datec, datep, datev, amount, fk_typepayment, \
          num_payment, fk_bank) \
         VALUES (:entity, :salaire, NOW(), :datep, :datev, :montant, :moyen, \
          :numero, :compte)",
        params! {
            "entity" => entity,
            "salaire" => rowid,
            "datep" => jour,
            "datev" => jour,
            "montant" => montant.to_string(),
            "moyen" => fk_typepayment,
            "numero" => numero,
            "compte" => fk_bank,
        },
    )?;
    let paiement = tx.last_insert_id().unwrap_or(0) as i64;

    // Le `AND paye = 0` double le verrou : si une autre transaction a validé
    // entre-temps, on n'écrase pas son versement, on annule le nôtre.
    tx.exec_drop(
        "UPDATE llx_salary SET paye = :paye, fk_typepayment = :moyen, \
         num_payment = :numero, fk_account = :compte \
         WHERE entity = :entity AND rowid = :rowid AND paye = 0",
        params! {
            "paye" => STATUT_PAYE,
            "moyen" => fk_typepayment,
            "numero" => numero,
            "compte" => fk_bank,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    if tx.affected_rows() == 0 {
        tx.rollback()?;
        return Err(format!("le salaire {rowid} vient d'être payé par un autre appel").into());
    }

    tx.commit()?;
    Ok(paiement)
}

/// Supprime un salaire **non payé**.
///
/// Un salaire payé ne se supprime pas : le versement existe, il justifie une
/// sortie de trésorerie, et l'effacer laisserait une ligne de paiement orpheline
/// dans `llx_payment_salary` (aucune clé étrangère ne la retient). Le circuit
/// correct est de solder l'erreur par un second bulletin d'ajustement, jamais de
/// réécrire le passé.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let mut tx = conn.start_transaction(TxOpts::default())?;

    let paye: Option<i64> = tx.exec_first(
        "SELECT paye FROM llx_salary WHERE entity = :entity AND rowid = :rowid FOR UPDATE",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(paye) = paye else {
        tx.rollback()?;
        return Err(format!("le salaire {rowid} n'existe pas dans l'entité {entity}").into());
    };
    if paye != i64::from(STATUT_NON_PAYE) {
        tx.rollback()?;
        return Err(format!(
            "le salaire {rowid} est payé : un versement constaté ne se supprime pas"
        )
        .into());
    }

    // Ceinture et bretelles : un versement partiel laisse `paye = 0` tout en
    // ayant écrit une ligne de paiement. La supprimer avec la fiche effacerait
    // une sortie d'argent réelle.
    let versements: Option<i64> = tx.exec_first(
        "SELECT COUNT(*) FROM llx_payment_salary WHERE entity = :entity AND fk_salary = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if versements.unwrap_or(0) > 0 {
        tx.rollback()?;
        return Err(format!(
            "le salaire {rowid} porte {} versement(s) enregistré(s) : supprimez-les \
             avant la fiche",
            versements.unwrap_or(0)
        )
        .into());
    }

    tx.exec_drop(
        "DELETE FROM llx_salary WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Total des salaires enregistrés pour une année civile, arrondi au centime.
///
/// L'année est celle de la **date de paiement**, à défaut celle de la création
/// de la fiche : la première est celle que Dolibarr trie et affiche, la seconde
/// est toujours renseignée (le noyau lui-même y met `NOW()`), ce qui évite de
/// faire disparaître du total les fiches qu'il a créées sans `datep`.
///
/// On somme **ce qui est enregistré**, pas seulement ce qui est sorti : un
/// filtre sur `paye = 1` donnerait un total qui change tout seul le jour du
/// virement, alors que la question posée est « quelle masse salariale ai-je
/// engagée sur l'exercice ». Les deux chiffres sont utiles, mais ce n'est pas
/// le même, et celui-ci est reproductible.
///
/// Le total est arrondi une seule fois, à la fin : arrondir chaque bulletin
/// avant de sommer ferait dériver le total d'un centime par ligne.
pub fn total_par_annee(pool: &Pool, entity: i64, annee: i32) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let total: Option<String> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(s.amount), 0) AS CHAR) FROM llx_salary s \
         WHERE s.entity = :entity AND YEAR(COALESCE(s.datep, DATE(s.datec))) = :annee",
        params! { "entity" => entity, "annee" => annee },
    )?;
    Ok(round_money(
        total
            .and_then(|t| Decimal::from_str(t.trim()).ok())
            .unwrap_or_default(),
    ))
}

/// Vérifie qu'une date `AAAA-MM-JJ` existe réellement.
///
/// `2026-02-30`, `2026-1-1` et `2026-13-01` sont refusés. Le format est exigé
/// tel quel : c'est celui que Dolibarr stocke, et sur cette forme l'ordre
/// alphabétique est l'ordre chronologique — ce qui permet de comparer les dates
/// sans les convertir, donc sans risque de fuseau ni de calendrier.
///
/// La colonne est un `date` : MariaDB accepterait `0000-00-00` ou tronquerait
/// silencieusement une valeur douteuse. Une date de paie fausse ne se voit pas
/// à la saisie ; elle se voit au bilan.
fn analyser_date(valeur: &str) -> Result<(), Box<dyn Error>> {
    let texte = valeur.trim();
    let octets = texte.as_bytes();
    let forme = octets.len() == 10
        && octets[4] == b'-'
        && octets[7] == b'-'
        && octets[..4].iter().all(u8::is_ascii_digit)
        && octets[5..7].iter().all(u8::is_ascii_digit)
        && octets[8..].iter().all(u8::is_ascii_digit);
    if !forme {
        return Err(format!("date « {valeur} » mal formée : AAAA-MM-JJ attendu").into());
    }
    let annee: i32 = texte[..4].parse()?;
    let mois: u32 = texte[5..7].parse()?;
    let jour: u32 = texte[8..].parse()?;
    if !(1..=12).contains(&mois) || jour == 0 || jour > jours_du_mois(annee, mois) {
        return Err(format!("la date « {texte} » n'existe pas dans le calendrier").into());
    }
    Ok(())
}

/// Nombre de jours d'un mois, par table : aucune boucle, aucun oubli de février.
fn jours_du_mois(annee: i32, mois: u32) -> u32 {
    match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bissextile(annee) => 29,
        2 => 28,
        // Mois hors 1-12 : déjà refusé par l'appelant, mais on ne rend pas 31
        // par accident.
        _ => 0,
    }
}

/// Règle grégorienne, y compris les siècles : 1900 n'est pas bissextile, 2000
/// l'est.
fn bissextile(annee: i32) -> bool {
    (annee % 4 == 0 && annee % 100 != 0) || annee % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une date valide passe, y compris le 29 février d'une année bissextile.
    #[test]
    fn accepte_les_dates_qui_existent() {
        assert!(analyser_date("2026-11-06").is_ok());
        assert!(analyser_date("  2026-01-31 ").is_ok());
        assert!(analyser_date("2024-02-29").is_ok());
        assert!(analyser_date("2000-02-29").is_ok());
    }

    /// Les trois pièges classiques : jour inexistant, forme libre, mois 13.
    #[test]
    fn refuse_les_dates_qui_n_existent_pas() {
        assert!(analyser_date("2026-02-30").is_err());
        assert!(analyser_date("2026-02-29").is_err());
        assert!(analyser_date("1900-02-29").is_err());
        assert!(analyser_date("2026-13-01").is_err());
        assert!(analyser_date("2026-00-10").is_err());
        assert!(analyser_date("2026-11-00").is_err());
        assert!(analyser_date("2026-1-1").is_err());
        assert!(analyser_date("06/11/2026").is_err());
        assert!(analyser_date("").is_err());
    }

    /// La référence retombe sur le `rowid` quand la colonne est vide : c'est le
    /// cas de tout salaire écrit par le noyau PHP, qui ne remplit jamais `ref`.
    #[test]
    fn la_reference_est_le_rowid_quand_la_colonne_est_vide() {
        assert_eq!(reference(None, 42), "42");
        assert_eq!(reference(Some("   ".into()), 42), "42");
        assert_eq!(reference(Some("42".into()), 42), "42");
        assert_eq!(reference(Some("7".into()), 42), "7");
    }
}
