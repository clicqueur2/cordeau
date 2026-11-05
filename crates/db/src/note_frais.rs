//! Notes de frais (`llx_expensereport`, `llx_expensereport_det`).
//!
//! Une note de frais, c'est ce qu'un salarié déclare avoir avancé sur une
//! période : un auteur, des dates de début et de fin, et des lignes de dépense
//! prises dans le dictionnaire national `llx_c_type_fees` (péage, repas,
//! hôtel…). Elle suit un circuit verrouillé — brouillon, en attente de
//! validation, approuvée ou refusée — et chaque transition laisse sa date en
//! base : c'est elle qui fait foi si le montant est contesté plus tard.
//!
//! Les montants sont des `double(24,8)` en base. Ils sortent en
//! `CAST(... AS CHAR)` puis passent par `Decimal::from_str` : jamais par un
//! flottant, sans quoi les totaux d'en-tête et la somme des lignes divergent au
//! centime.

use std::error::Error;
use std::str::FromStr;

use money::{line_totals, sum, LineInput};
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

use crate::divers;

/// Préfixe de référence des notes de frais.
pub const PREFIX: &str = "NF";

/// Brouillon : modifiable et supprimable par son auteur.
pub const STATUT_BROUILLON: i16 = 0;
/// En attente de validation par un responsable.
pub const STATUT_ATTENTE_VALIDATION: i16 = 2;
/// Approuvée : les frais sont dus au salarié.
pub const STATUT_APPROUVEE: i16 = 5;
/// Payée : le remboursement est sorti.
pub const STATUT_PAYEE: i16 = 6;
/// Refusée : la note ne sera pas remboursée.
pub const STATUT_REFUSEE: i16 = 99;

/// Taille de `llx_expensereport.detail_refuse`, en octets.
const MOTIF_MAX: usize = 255;

/// Modèle PDF par défaut de Dolibarr pour les notes de frais
/// (`mod_expensereport_sponge`). Sans modèle, la note n'est pas imprimable.
const MODELE_PDF: &str = "sponge";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteFrais {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    pub fk_user_author: i64,
    /// Login de l'auteur (`llx_user.login`).
    pub auteur: Option<String>,
    pub date_debut: String,
    pub date_fin: String,
    pub date_create: Option<String>,
    pub date_valid: Option<String>,
    pub date_approve: Option<String>,
    pub date_refuse: Option<String>,
    pub statut: i16,
    pub paid: i16,
    pub total_ht: Decimal,
    pub total_tva: Decimal,
    pub total_ttc: Decimal,
    pub note_public: Option<String>,
    pub note_private: Option<String>,
    /// Motif du refus, tel qu'il a été saisi.
    pub detail_refuse: Option<String>,
    pub fk_c_paiement: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LigneFrais {
    pub rowid: i64,
    pub fk_expensereport: i64,
    pub fk_c_type_fees: i64,
    /// Libellé du type de frais (`llx_c_type_fees.label`).
    pub type_frais: Option<String>,
    pub date: String,
    pub comments: String,
    pub qty: Decimal,
    pub subprice: Decimal,
    pub value_unit: Decimal,
    pub tva_tx: Decimal,
    pub total_ht: Decimal,
    pub total_tva: Decimal,
    pub total_ttc: Decimal,
    pub fk_projet: Option<i64>,
    pub rang: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewNoteFrais {
    pub fk_user_author: i64,
    pub date_debut: String,
    pub date_fin: String,
    pub note_public: Option<String>,
    pub note_private: Option<String>,
    pub lines: Vec<NewLigneFrais>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewLigneFrais {
    pub fk_c_type_fees: i64,
    pub date: String,
    pub comments: String,
    pub qty: Decimal,
    /// Prix unitaire hors taxes, comme sur un devis ou une facture.
    pub subprice: Decimal,
    pub tva_tx: Decimal,
}

const SELECT_COLS: &str = "e.rowid, e.entity, e.ref, e.fk_user_author, u.login, \
     DATE_FORMAT(e.date_debut, '%Y-%m-%d'), DATE_FORMAT(e.date_fin, '%Y-%m-%d'), \
     DATE_FORMAT(e.date_create, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(e.date_valid, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(e.date_approve, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(e.date_refuse, '%Y-%m-%d %H:%i:%s'), \
     e.fk_statut, e.paid, CAST(e.total_ht AS CHAR), CAST(e.total_tva AS CHAR), \
     CAST(e.total_ttc AS CHAR), e.note_public, e.note_private, e.detail_refuse, \
     e.fk_c_paiement";

/// `llx_expensereport_det` ne porte pas de colonne `entity` : le cloisonnement
/// passe par la jointure sur l'en-tête, d'où les alias.
const LINE_COLS: &str = "d.rowid, d.fk_expensereport, d.fk_c_type_fees, t.label, \
     DATE_FORMAT(d.date, '%Y-%m-%d'), d.comments, CAST(d.qty AS CHAR), \
     CAST(d.subprice AS CHAR), CAST(d.value_unit AS CHAR), CAST(d.tva_tx AS CHAR), \
     CAST(d.total_ht AS CHAR), CAST(d.total_tva AS CHAR), CAST(d.total_ttc AS CHAR), \
     d.fk_projet, d.rang";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: &Row) -> NoteFrais {
    NoteFrais {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2).unwrap_or_default(),
        fk_user_author: opt_i(row, 3).unwrap_or(0),
        auteur: opt_s(row, 4),
        date_debut: opt_s(row, 5).unwrap_or_default(),
        date_fin: opt_s(row, 6).unwrap_or_default(),
        date_create: opt_s(row, 7),
        date_valid: opt_s(row, 8),
        date_approve: opt_s(row, 9),
        date_refuse: opt_s(row, 10),
        statut: opt_i(row, 11).unwrap_or(0) as i16,
        paid: opt_i(row, 12).unwrap_or(0) as i16,
        total_ht: opt_dec(row, 13).unwrap_or_default(),
        total_tva: opt_dec(row, 14).unwrap_or_default(),
        total_ttc: opt_dec(row, 15).unwrap_or_default(),
        note_public: opt_s(row, 16),
        note_private: opt_s(row, 17),
        detail_refuse: opt_s(row, 18),
        fk_c_paiement: opt_i(row, 19),
    }
}

fn map_line(row: &Row) -> LigneFrais {
    LigneFrais {
        rowid: opt_i(row, 0).unwrap_or(0),
        fk_expensereport: opt_i(row, 1).unwrap_or(0),
        fk_c_type_fees: opt_i(row, 2).unwrap_or(0),
        type_frais: opt_s(row, 3),
        date: opt_s(row, 4).unwrap_or_default(),
        comments: opt_s(row, 5).unwrap_or_default(),
        qty: opt_dec(row, 6).unwrap_or_default(),
        subprice: opt_dec(row, 7).unwrap_or_default(),
        value_unit: opt_dec(row, 8).unwrap_or_default(),
        tva_tx: opt_dec(row, 9).unwrap_or_default(),
        total_ht: opt_dec(row, 10).unwrap_or_default(),
        total_tva: opt_dec(row, 11).unwrap_or_default(),
        total_ttc: opt_dec(row, 12).unwrap_or_default(),
        fk_projet: opt_i(row, 13),
        rang: opt_i(row, 14).unwrap_or(0) as i32,
    }
}

/// Notes de frais d'une entité, de la plus récente à la plus ancienne.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<NoteFrais>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_expensereport e \
         LEFT JOIN llx_user u ON u.rowid = e.fk_user_author \
         WHERE e.entity = :entity \
         ORDER BY e.date_debut DESC, e.rowid DESC LIMIT :limite OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_row).collect())
}

/// Une note de frais précise, ou `None` si elle n'existe pas dans l'entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<NoteFrais>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_expensereport e \
         LEFT JOIN llx_user u ON u.rowid = e.fk_user_author \
         WHERE e.entity = :entity AND e.rowid = :rowid"
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_row))
}

/// Lignes de dépense d'une note, dans l'ordre de saisie.
pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_expensereport: i64,
) -> Result<Vec<LigneFrais>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_expensereport_det d \
         INNER JOIN llx_expensereport e ON e.rowid = d.fk_expensereport \
         LEFT JOIN llx_c_type_fees t ON t.id = d.fk_c_type_fees \
         WHERE e.entity = :entity AND d.fk_expensereport = :note \
         ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "note" => fk_expensereport },
    )?;
    Ok(rows.iter().map(map_line).collect())
}

/// Totaux d'une ligne, arrondis au centime.
///
/// Le TTC est arrondi puis la TVA est prise par différence : c'est le calcul de
/// `price.lib.php`, celui que le crate `money` reproduit (`round_money` =
/// `round_dp(2)` demi hors de zéro). Calculer `tva = ht × taux` donnerait 3,00
/// sur 29,99 à 10 % et casserait l'égalité `ht + tva = ttc`.
fn totaux_ligne(qty: Decimal, subprice: Decimal, tva_tx: Decimal) -> (Decimal, Decimal, Decimal) {
    let t = line_totals(&LineInput::new(
        &qty.to_string(),
        &subprice.to_string(),
        "0",
        &tva_tx.to_string(),
    ));
    (t.total_ht, t.total_vat, t.total_ttc)
}

/// Vérifie une date `AAAA-MM-JJ` et la rend comparable.
///
/// Sans ce contrôle, une date mal formée partirait telle quelle vers MySQL : en
/// mode strict l'insertion échoue sur un message de pilote incompréhensible, en
/// mode permissif elle écrirait `0000-00-00` dans la période de la note.
fn lire_date(valeur: &str, champ: &str) -> Result<(i32, u32, u32), Box<dyn Error>> {
    let morceaux: Vec<&str> = valeur.trim().split('-').collect();
    let lu = match morceaux.as_slice() {
        [annee, mois, jour] => match (
            annee.parse::<i32>(),
            mois.parse::<u32>(),
            jour.parse::<u32>(),
        ) {
            (Ok(a), Ok(m), Ok(j)) if (1..=12).contains(&m) && (1..=31).contains(&j) => {
                Some((a, m, j))
            }
            _ => None,
        },
        _ => None,
    };
    match lu {
        Some(date) => Ok(date),
        None => {
            Err(format!("{champ} : « {valeur} » n'est pas une date au format AAAA-MM-JJ").into())
        }
    }
}

/// Contrôles qui ne demandent pas la base : contenu et chronologie.
fn verifier_entree(new: &NewNoteFrais) -> Result<(), Box<dyn Error>> {
    if new.fk_user_author <= 0 {
        return Err("l'auteur de la note de frais (fk_user_author) est obligatoire".into());
    }
    if new.lines.is_empty() {
        return Err("une note de frais doit comporter au moins une ligne".into());
    }
    let debut = lire_date(&new.date_debut, "date de début")?;
    let fin = lire_date(&new.date_fin, "date de fin")?;
    if fin < debut {
        return Err(format!(
            "la date de fin ({}) ne peut pas précéder la date de début ({})",
            new.date_fin.trim(),
            new.date_debut.trim()
        )
        .into());
    }
    for (i, l) in new.lines.iter().enumerate() {
        if l.qty <= Decimal::ZERO {
            return Err(format!(
                "ligne {} : la quantité doit être strictement positive",
                i + 1
            )
            .into());
        }
        lire_date(&l.date, &format!("ligne {} : date", i + 1))?;
    }
    Ok(())
}

/// Le type de frais existe-t-il, et est-il encore proposé ?
///
/// Un type désactivé reste en base (les anciennes notes doivent continuer de
/// s'afficher) mais ne doit plus servir à en créer une : accepter un id inconnu
/// ferait une ligne que Dolibarr ne saurait ni libeller ni comptabiliser.
fn verifier_type_frais(conn: &mut mysql::PooledConn, id: i64) -> Result<(), Box<dyn Error>> {
    let actif: Option<i64> = conn.exec_first(
        "SELECT active FROM llx_c_type_fees WHERE id = :id",
        params! { "id" => id },
    )?;
    match actif {
        None => Err(format!("le type de frais {id} n'existe pas").into()),
        Some(0) => Err(format!("le type de frais {id} est désactivé").into()),
        Some(_) => Ok(()),
    }
}

/// Une ligne prête à être écrite, totaux déjà calculés.
struct Ligne {
    fk_c_type_fees: i64,
    date: String,
    comments: String,
    qty: Decimal,
    subprice: Decimal,
    tva_tx: Decimal,
    total_ht: Decimal,
    total_tva: Decimal,
    total_ttc: Decimal,
}

/// Contrôles qui demandent la base : l'auteur et les types de frais existent-ils
/// vraiment ?
///
/// Ouvert sur sa propre connexion, refermée aussitôt : la création a besoin
/// d'une seconde connexion pour la numérotation, et garder les deux ouvertes en
/// même temps épuise le pool quand plusieurs notes se créent de front.
fn verifier_references(pool: &Pool, entity: i64, new: &NewNoteFrais) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;

    // `llx_user` n'a pas de clé étrangère vers les notes : un auteur fantôme
    // passerait sans bruit et la note n'apparaîtrait dans l'écran de personne.
    let auteur: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_user WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => new.fk_user_author },
    )?;
    if auteur.is_none() {
        return Err(format!(
            "l'utilisateur {} n'existe pas dans l'entité {}",
            new.fk_user_author, entity
        )
        .into());
    }

    for l in &new.lines {
        verifier_type_frais(&mut conn, l.fk_c_type_fees)?;
    }
    Ok(())
}

/// Calcule les lignes à écrire : totaux compris.
fn preparer_lignes(new: &NewNoteFrais) -> Vec<Ligne> {
    new.lines
        .iter()
        .map(|l| {
            let (total_ht, total_tva, total_ttc) = totaux_ligne(l.qty, l.subprice, l.tva_tx);
            Ligne {
                fk_c_type_fees: l.fk_c_type_fees,
                date: l.date.trim().to_string(),
                comments: l.comments.clone(),
                qty: l.qty,
                subprice: l.subprice,
                tva_tx: l.tva_tx,
                total_ht,
                total_tva,
                total_ttc,
            }
        })
        .collect()
}

/// Crée une note de frais en brouillon et renvoie son `rowid`.
pub fn create(pool: &Pool, entity: i64, new: &NewNoteFrais) -> Result<i64, Box<dyn Error>> {
    verifier_entree(new)?;
    verifier_references(pool, entity, new)?;

    let lignes = preparer_lignes(new);

    // Les totaux d'en-tête somment les lignes déjà arrondies, puis arrondissent
    // une seule fois : sommer des montants bruts ferait diverger l'en-tête et le
    // détail à l'affichage.
    let hts: Vec<Decimal> = lignes.iter().map(|l| l.total_ht).collect();
    let tvas: Vec<Decimal> = lignes.iter().map(|l| l.total_tva).collect();
    let ttcs: Vec<Decimal> = lignes.iter().map(|l| l.total_ttc).collect();
    let total_ht = sum(&hts);
    let total_tva = sum(&tvas);
    let total_ttc = sum(&ttcs);

    let racine = divers::suivant_ref(pool, "llx_expensereport", "ref", PREFIX, entity)?;
    let mut conn = pool.get_conn()?;
    // Cinq essais : au-delà, ce n'est plus une collision de numérotation mais un
    // problème de base, et insister ne ferait que retarder le message d'erreur.
    for essai in 0..5u32 {
        let reference = decaler_ref(&racine, essai);
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let insere = tx.exec_drop(
            "INSERT INTO llx_expensereport \
             (entity, ref, fk_user_author, date_debut, date_fin, date_create, \
              fk_statut, paid, total_ht, total_tva, total_ttc, note_public, \
              note_private, model_pdf) \
             VALUES (:entity, :ref, :auteur, :debut, :fin, NOW(), :statut, 0, \
              :total_ht, :total_tva, :total_ttc, :note_public, :note_private, \
              :modele)",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "auteur" => new.fk_user_author,
                "debut" => new.date_debut.trim(),
                "fin" => new.date_fin.trim(),
                "statut" => STATUT_BROUILLON,
                "total_ht" => total_ht.to_string(),
                "total_tva" => total_tva.to_string(),
                "total_ttc" => total_ttc.to_string(),
                "note_public" => new.note_public.as_deref(),
                "note_private" => new.note_private.as_deref(),
                "modele" => MODELE_PDF,
            },
        );
        match insere {
            Ok(()) => {
                let rowid = tx.last_insert_id().unwrap_or(0) as i64;
                inserer_lignes(&mut tx, rowid, &lignes)?;
                tx.commit()?;
                return Ok(rowid);
            }
            Err(e) if is_duplicate(&e) && essai < 4 => {
                // Un autre appel a pris la référence entre-temps : on annule et
                // on repart avec la valeur suivante.
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une référence de note de frais unique".into())
}

/// Avance la référence d'un cran par essai : `NF0042` devient `NF0043`.
///
/// `divers::suivant_ref` relit le maximum avant chaque insertion ; deux notes
/// créées en même temps tombent sur la même valeur et l'index unique
/// `(ref, entity)` refuse la seconde. Plutôt que d'espérer une relecture plus
/// fraîche, on avance le compteur à la main.
fn decaler_ref(racine: &str, decalage: u32) -> String {
    if decalage == 0 {
        return racine.to_string();
    }
    let suffixe = racine.strip_prefix(PREFIX).unwrap_or(racine);
    match suffixe.parse::<u32>() {
        Ok(n) => format!("{PREFIX}{:04}", n + decalage),
        // Référence d'une autre forme (reprise d'un ancien paramétrage) : on ne
        // devine pas sa structure, l'insertion tranchera.
        Err(_) => racine.to_string(),
    }
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

fn inserer_lignes(
    tx: &mut mysql::Transaction<'_>,
    fk_expensereport: i64,
    lignes: &[Ligne],
) -> Result<(), Box<dyn Error>> {
    for (i, l) in lignes.iter().enumerate() {
        tx.exec_drop(
            // `value_unit` est NOT NULL sans défaut : on y remet le prix
            // unitaire HT saisi, faute d'un second prix dans la note.
            "INSERT INTO llx_expensereport_det \
             (fk_expensereport, fk_c_type_fees, comments, qty, subprice, \
              value_unit, remise_percent, vat_src_code, tva_tx, total_ht, \
              total_tva, total_ttc, date, rang) \
             VALUES (:note, :type_frais, :comments, :qty, :subprice, :subprice, \
              0, '', :tva_tx, :total_ht, :total_tva, :total_ttc, :date, :rang)",
            params! {
                "note" => fk_expensereport,
                "type_frais" => l.fk_c_type_fees,
                "comments" => l.comments.as_str(),
                "qty" => l.qty.to_string(),
                "subprice" => l.subprice.to_string(),
                "tva_tx" => l.tva_tx.to_string(),
                "total_ht" => l.total_ht.to_string(),
                "total_tva" => l.total_tva.to_string(),
                "total_ttc" => l.total_ttc.to_string(),
                "date" => l.date.as_str(),
                "rang" => (i + 1) as i32,
            },
        )?;
    }
    Ok(())
}

/// Statut courant d'une note, vue par l'entité qui la demande.
fn statut_actuel(
    conn: &mut mysql::PooledConn,
    entity: i64,
    rowid: i64,
) -> Result<i16, Box<dyn Error>> {
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_expensereport WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        Some(s) => Ok(s as i16),
        None => Err(format!("note de frais {rowid} introuvable dans l'entité {entity}").into()),
    }
}

/// Soumet une note : brouillon (0) → en attente de validation (2).
///
/// `fk_user_valid` reste vide : la signature du valideur ne se pose pas ici,
/// faute de savoir qui valide. Mieux vaut une colonne honnêtement vide qu'un
/// nom faux dans un document qui sert de preuve.
pub fn soumettre(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut == STATUT_ATTENTE_VALIDATION {
        return Err("cette note de frais est déjà en attente de validation".into());
    }
    if statut != STATUT_BROUILLON {
        return Err(format!(
            "seule une note de frais en brouillon peut être soumise (statut actuel : {statut})"
        )
        .into());
    }
    conn.exec_drop(
        "UPDATE llx_expensereport SET fk_statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_ATTENTE_VALIDATION,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Approuve une note : en attente (2) → approuvée (5).
///
/// Une note encore en brouillon n'a pas de `date_valid` : l'approuver
/// court-circuiterait la validation, donc on la refuse.
pub fn approuver(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut == STATUT_APPROUVEE {
        return Err("cette note de frais est déjà approuvée".into());
    }
    if statut != STATUT_ATTENTE_VALIDATION {
        return Err(format!(
            "seule une note de frais soumise peut être approuvée (statut actuel : {statut})"
        )
        .into());
    }
    conn.exec_drop(
        "UPDATE llx_expensereport SET fk_statut = :statut, date_approve = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_APPROUVEE,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Refuse une note : en attente (2) → refusée (99), avec son motif.
///
/// Un motif vide est écrit `NULL` plutôt qu'en chaîne vide : les écrans de
/// Dolibarr testent `empty()`, mais une colonne vide et une colonne sans motif
/// ne veulent pas dire la même chose pour un export.
pub fn refuser(pool: &Pool, entity: i64, rowid: i64, motif: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut == STATUT_REFUSEE {
        return Err("cette note de frais est déjà refusée".into());
    }
    if statut != STATUT_ATTENTE_VALIDATION {
        return Err(format!(
            "seule une note de frais soumise peut être refusée (statut actuel : {statut})"
        )
        .into());
    }
    let motif = motif.trim();
    let motif = if motif.is_empty() {
        None
    } else {
        Some(tronquer(motif, MOTIF_MAX))
    };
    conn.exec_drop(
        "UPDATE llx_expensereport SET fk_statut = :statut, date_refuse = NOW(), \
         detail_refuse = :motif WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_REFUSEE,
            "motif" => motif,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Borne un texte à `max` octets sans couper un caractère.
///
/// `detail_refuse` est un `varchar(255)` : un accent saisi en UTF-8 occupe deux
/// octets et une coupe au milieu laisserait un octet invalide, que MySQL
/// refuserait d'écrire en latin1.
fn tronquer(texte: &str, max: usize) -> &str {
    if texte.len() <= max {
        return texte;
    }
    let mut fin = max;
    while fin > 0 && !texte.is_char_boundary(fin) {
        fin -= 1;
    }
    &texte[..fin]
}

/// Supprime une note en brouillon, lignes comprises.
///
/// Une note soumise est refusée : elle est peut-être déjà dans la file d'un
/// valideur, et la faire disparaître sous ses yeux ferait perdre la trace du
/// refus ou de l'accord.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut = statut_actuel(&mut conn, entity, rowid)?;
    if statut != STATUT_BROUILLON {
        return Err(format!(
            "seule une note de frais en brouillon peut être supprimée (statut actuel : {statut})"
        )
        .into());
    }
    let mut tx = conn.start_transaction(TxOpts::default())?;
    // `llx_expensereport_det` n'a pas d'`entity` : le `rowid` de l'en-tête, lui,
    // est unique, et c'est la seule clé dont les lignes dépendent.
    tx.exec_drop(
        "DELETE FROM llx_expensereport_det WHERE fk_expensereport = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_expensereport WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}
