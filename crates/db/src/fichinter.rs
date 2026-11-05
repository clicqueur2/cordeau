//! Interventions, autrement dit bons de travail (`llx_fichinter` + `llx_fichinterdet`).
//!
//! Une intervention raconte un déplacement : « je suis passé chez le client,
//! voilà ce que j'ai fait et combien de temps ça a pris ». Deux tables :
//!
//! - `llx_fichinter` : l'en-tête — tiers, date, durée, statut, notes ;
//! - `llx_fichinterdet` : les lignes, une par tâche accomplie sur place.
//!
//! Une intervention **peut n'avoir aucune ligne**. C'est alors l'en-tête et sa
//! description qui racontent le déplacement : l'artisan qui vient constater une
//! fuite n'a qu'une phrase à écrire, et lui imposer une ligne de détail serait
//! une corvée inventée par le logiciel.
//!
//! ## Statuts
//!
//! `llx_fichinter.fk_statut` : `0` brouillon, `1` validée, `2` clôturée. Ce
//! n'est pas décoratif : un brouillon est la seule chose qu'on supprime
//! ([`supprimer`]). Une intervention validée a été imprimée et remise au client,
//! sa référence ne doit plus disparaître — elle se clôture ([`cloturer`]).
//!
//! ## Unité de durée : la minute
//!
//! **Les deux colonnes `duree` comptent des minutes**, et c'est l'unité que ce
//! module écrit et rend partout ([`duree_totale`]). L'en-tête
//! (`llx_fichinter.duree`, type `real`) admet des fractions de minute — une
//! intervention de 1 h 30 s'écrit `90` ou `90.5` ; la ligne
//! (`llx_fichinterdet.duree`, type `integer`) est entière, ce qui suffit à une
//! tâche de chantier. Dolibarr n'écrit l'unité nulle part dans son schéma : elle
//! est fixée ici une fois pour toutes, plutôt que laissée à deviner par chaque
//! appelant — un `90` relu comme des heures ferait un devis faux.
//!
//! ## Numérotation
//!
//! `FI` + compteur, par [`crate::divers::suivant_ref`]. Le préfixe est celui de
//! Dolibarr. Réserve à connaître : `suivant_ref` lit la **queue numérique** de
//! la référence, donc une base déjà peuplée de références Dolibarr
//! `FI<AAMM>-<NNNN>` repart du numéro d'année-mois (`FI2611` par exemple) au
//! lieu de `FI2611-0001`. C'est laid, ce n'est pas faux : l'unicité est tenue
//! par l'index `uk_fichinter_ref (ref, entity)`, et une collision est rejouée,
//! jamais écrasée.
//!
//! ## Colonnes vérifiées sur le DDL de Dolibarr 24.0.1
//!
//! - `fk_soc` est `NOT NULL` et porte `fk_fichinter_fk_soc` : le tiers doit
//!   exister. C'est le seul refus que le schéma impose.
//! - `fk_projet` vaut `0` chez Dolibarr quand aucun projet n'est rattaché. Ici
//!   l'appelant donne `None`, la colonne reçoit `0`, la lecture le rend `None` :
//!   l'absence de projet n'a qu'une seule représentation côté appelant. Aucune
//!   clé étrangère ne contraint cette colonne.
//! - `fk_user_author` **n'a aucune clé étrangère**, contrairement à
//!   `llx_contrat.fk_user_author` où la valeur `0` de Dolibarr est refusée par
//!   MySQL : `NULL` est donc licite, et c'est ce qu'on écrit quand l'appelant
//!   ne précise pas d'auteur.
//! - `llx_fichinterdet` n'a **pas** de colonne `entity` : le cloisonnement passe
//!   par la jointure sur `llx_fichinter.entity`. Aucune ligne ne se lit ni ne
//!   s'écrit hors de l'entité de son en-tête.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

/// Préfixe des références d'intervention, celui de Dolibarr.
pub const PREFIXE: &str = "FI";

/// Brouillon : modifiable, supprimable, pas encore remis au client.
pub const STATUT_BROUILLON: i16 = 0;
/// Validée : imprimée, la référence est acquise.
pub const STATUT_VALIDE: i16 = 1;
/// Clôturée : le travail est terminé et daté.
pub const STATUT_CLOTURE: i16 = 2;

/// Une intervention telle qu'elle est en base.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fichinter {
    pub rowid: i64,
    pub entity: i64,
    pub ref_: String,
    /// Tiers chez qui l'on est passé (`fk_soc` est `NOT NULL`).
    pub fk_soc: i64,
    /// Nom du tiers, ramené par la jointure de [`list`] et [`get`].
    pub tiers: Option<String>,
    pub fk_projet: Option<i64>,
    pub ref_client: Option<String>,
    /// Date de saisie (`datec`, `datetime`).
    pub datec: Option<String>,
    /// Date de l'intervention (`datei`, `date`).
    pub datei: Option<String>,
    /// Date de validation (`date_valid`, `datetime`).
    pub date_valid: Option<String>,
    pub dateo: Option<String>,
    /// Date de fin (`datee`, `date`), posée par [`cloturer`].
    pub datee: Option<String>,
    pub datet: Option<String>,
    /// Durée de l'en-tête, **en minutes** (voir la doc du module).
    pub duree: Option<Decimal>,
    pub statut: i16,
    pub description: Option<String>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub model_pdf: Option<String>,
}

impl Fichinter {
    pub fn est_brouillon(&self) -> bool {
        self.statut == STATUT_BROUILLON
    }

    pub fn est_validee(&self) -> bool {
        self.statut == STATUT_VALIDE
    }

    pub fn est_cloturee(&self) -> bool {
        self.statut == STATUT_CLOTURE
    }
}

/// Une ligne d'intervention : une tâche, avec sa durée en minutes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FichinterLine {
    pub rowid: i64,
    pub fk_fichinter: i64,
    /// Date de la tâche (`datetime`, ramenée à la minute).
    pub date: Option<String>,
    pub description: Option<String>,
    /// Durée **en minutes** (`integer`).
    pub duree: i64,
    /// Ordre d'affichage sur le bon de travail, à partir de 1.
    pub rang: i64,
}

/// Ligne à créer. Une `date` vide prend la date de l'intervention.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewFichinterLine {
    pub date: Option<String>,
    pub description: String,
    pub duree: i64,
}

impl NewFichinterLine {
    pub fn new(description: &str, duree: i64) -> Self {
        Self {
            description: description.to_string(),
            duree,
            ..Self::default()
        }
    }
}

/// Intervention à créer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewFichinter {
    /// Tiers chez qui l'intervention a lieu. Obligatoire.
    pub fk_soc: i64,
    /// Projet rattaché, facultatif : écrit `0` en base comme Dolibarr.
    pub fk_projet: Option<i64>,
    /// Référence du client (« bon de commande », numéro d'affaire…).
    pub ref_client: Option<String>,
    /// Date de l'intervention (`datei`). Obligatoire : une intervention sans
    /// date ne se classe nulle part et ne se facture pas.
    pub datei: String,
    /// Ce qui a été fait, en clair. Facultatif : une intervention peut se
    /// raconter par ses lignes.
    pub description: String,
    /// Durée totale **en minutes**, en plus de celle des lignes.
    pub duree: Option<Decimal>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    /// Auteur de la saisie. `None` laisse la colonne à `NULL` (aucune clé
    /// étrangère ici, cf. la doc du module).
    pub fk_user_author: Option<i64>,
    /// Lignes de détail. Vide = intervention simple : c'est permis.
    pub lines: Vec<NewFichinterLine>,
}

impl NewFichinter {
    pub fn new(fk_soc: i64) -> Self {
        Self {
            fk_soc,
            ..Self::default()
        }
    }
}

const SELECT_COLS: &str = "f.rowid, f.entity, f.ref, f.fk_soc, s.nom, \
     NULLIF(f.fk_projet, 0), f.ref_client, \
     DATE_FORMAT(f.datec, '%Y-%m-%d %H:%i'), DATE_FORMAT(f.datei, '%Y-%m-%d'), \
     DATE_FORMAT(f.date_valid, '%Y-%m-%d %H:%i'), DATE_FORMAT(f.dateo, '%Y-%m-%d'), \
     DATE_FORMAT(f.datee, '%Y-%m-%d'), DATE_FORMAT(f.datet, '%Y-%m-%d'), \
     CAST(f.duree AS CHAR), f.fk_statut, f.description, f.note_private, \
     f.note_public, f.model_pdf";

const LINE_COLS: &str = "d.rowid, d.fk_fichinter, DATE_FORMAT(d.date, '%Y-%m-%d %H:%i'), \
     d.description, d.duree, d.rang";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

/// Un texte vide vaut `NULL`.
///
/// Dolibarr distingue « pas de note » (`NULL`) d'une note vide (`''`) : la
/// seconde se retape à la main dans un formulaire, la première laisse le champ
/// libre. On garde le texte tel quel quand il y a quelque chose dedans, sinon on
/// écrit `NULL`, pour que l'aller-retour `NewFichinter` vers [`Fichinter`] ne
/// rende jamais une chaîne vide là où l'appelant n'a rien dit.
fn texte_ou_null(texte: &str) -> Option<String> {
    if texte.trim().is_empty() {
        None
    } else {
        Some(texte.to_string())
    }
}

fn map_row(row: Row) -> Fichinter {
    Fichinter {
        rowid: opt_i(&row, 0).unwrap_or(0),
        entity: opt_i(&row, 1).unwrap_or(1),
        ref_: opt_s(&row, 2).unwrap_or_default(),
        fk_soc: opt_i(&row, 3).unwrap_or(0),
        tiers: opt_s(&row, 4),
        fk_projet: opt_i(&row, 5),
        ref_client: opt_s(&row, 6),
        datec: opt_s(&row, 7),
        datei: opt_s(&row, 8),
        date_valid: opt_s(&row, 9),
        dateo: opt_s(&row, 10),
        datee: opt_s(&row, 11),
        datet: opt_s(&row, 12),
        duree: opt_dec(&row, 13),
        statut: opt_i(&row, 14).unwrap_or(0) as i16,
        description: opt_s(&row, 15),
        note_private: opt_s(&row, 16),
        note_public: opt_s(&row, 17),
        model_pdf: opt_s(&row, 18),
    }
}

fn map_line(row: Row) -> FichinterLine {
    FichinterLine {
        rowid: opt_i(&row, 0).unwrap_or(0),
        fk_fichinter: opt_i(&row, 1).unwrap_or(0),
        date: opt_s(&row, 2),
        description: opt_s(&row, 3),
        duree: opt_i(&row, 4).unwrap_or(0),
        rang: opt_i(&row, 5).unwrap_or(0),
    }
}

/// Interventions de l'entité, de la plus récente à la plus ancienne.
///
/// Le tri porte sur `datec` (date de saisie) et non sur `datei` : deux
/// interventions saisies le même jour pour des dates différentes doivent se
/// classer dans l'ordre où elles ont été écrites, sinon la première ligne de la
/// liste change toute seule dès qu'une date passée est antédatée.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Fichinter>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_fichinter f \
         LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE f.entity = ? ORDER BY f.datec DESC, f.rowid DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<Row> = conn.exec(sql, (entity, limite, offset))?;
    Ok(rows.into_iter().map(map_row).collect())
}

/// Une intervention précise, avec le nom de son tiers.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Fichinter>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_fichinter f \
         LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
         WHERE f.entity = ? AND f.rowid = ?"
    );
    let row: Option<Row> = conn.exec_first(sql, (entity, rowid))?;
    Ok(row.map(map_row))
}

/// Lignes d'une intervention, dans l'ordre du bon de travail.
///
/// La jointure sur `llx_fichinter` n'est pas une précaution de style : la table
/// de détail ne porte pas `entity`, et sans elle une intervention d'une autre
/// entité livrerait ses lignes à qui connaît son identifiant.
pub fn lines(
    pool: &Pool,
    entity: i64,
    fk_fichinter: i64,
) -> Result<Vec<FichinterLine>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {LINE_COLS} FROM llx_fichinterdet d \
         INNER JOIN llx_fichinter f ON f.rowid = d.fk_fichinter \
         WHERE f.entity = :entity AND d.fk_fichinter = :fk \
         ORDER BY d.rang ASC, d.rowid ASC"
    );
    let rows: Vec<Row> = conn.exec(sql, params! { "entity" => entity, "fk" => fk_fichinter })?;
    Ok(rows.into_iter().map(map_line).collect())
}

/// Crée une intervention en brouillon, avec ses lignes s'il y en a.
///
/// Refuse un tiers inexistant, une date d'intervention vide et une durée
/// négative. Accepte en revanche une intervention **sans aucune ligne**.
///
/// En-tête et lignes entrent dans la même transaction : une intervention à
/// moitié écrite serait pire qu'une intervention absente. La référence suit
/// [`crate::divers::suivant_ref`] ; en cas de collision sur `uk_fichinter_ref`
/// (deux saisies simultanées), on rejoue avec le numéro suivant plutôt que
/// d'échouer.
pub fn create(pool: &Pool, entity: i64, new: &NewFichinter) -> Result<i64, Box<dyn Error>> {
    if new.fk_soc <= 0 {
        return Err("le tiers (fk_soc) est obligatoire".into());
    }
    let datei = new.datei.trim();
    if datei.is_empty() {
        return Err("la date de l'intervention est obligatoire".into());
    }
    if matches!(new.duree, Some(duree) if duree < Decimal::ZERO) {
        return Err("la durée de l'intervention ne peut pas être négative".into());
    }
    for ligne in &new.lines {
        if ligne.duree < 0 {
            return Err("la durée d'une ligne ne peut pas être négative".into());
        }
    }

    let mut conn = pool.get_conn()?;
    let tiers: Option<i64> = conn.exec_first(
        "SELECT rowid FROM llx_societe WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => new.fk_soc },
    )?;
    if tiers.is_none() {
        return Err(format!(
            "le tiers {} n'existe pas dans l'entité {}",
            new.fk_soc, entity
        )
        .into());
    }

    let description = texte_ou_null(&new.description);
    let ref_client = new.ref_client.as_deref().and_then(texte_ou_null);
    let note_private = new.note_private.as_deref().and_then(texte_ou_null);
    let note_public = new.note_public.as_deref().and_then(texte_ou_null);
    let duree = new.duree.map(|d| d.to_string());

    for essai in 0..5u32 {
        let reference = crate::divers::suivant_ref(pool, "llx_fichinter", "ref", PREFIXE, entity)?;
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let insere = tx.exec_drop(
            "INSERT INTO llx_fichinter \
             (entity, ref, fk_soc, fk_projet, ref_client, datec, datei, \
              fk_user_author, fk_statut, duree, description, note_private, note_public) \
             VALUES (:entity, :ref, :fk_soc, COALESCE(:fk_projet, 0), :ref_client, NOW(), \
              :datei, :auteur, :statut, :duree, :description, :note_private, :note_public)",
            params! {
                "entity" => entity,
                "ref" => reference.as_str(),
                "fk_soc" => new.fk_soc,
                "fk_projet" => new.fk_projet,
                "ref_client" => ref_client.as_deref(),
                "datei" => datei,
                "auteur" => new.fk_user_author,
                "statut" => STATUT_BROUILLON,
                "duree" => duree.as_deref(),
                "description" => description.as_deref(),
                "note_private" => note_private.as_deref(),
                "note_public" => note_public.as_deref(),
            },
        );
        match insere {
            Ok(()) => {
                let rowid = tx
                    .last_insert_id()
                    .ok_or("identifiant de l'intervention insérée indisponible")?
                    as i64;
                for (position, ligne) in new.lines.iter().enumerate() {
                    let texte = texte_ou_null(&ligne.description);
                    // Une ligne sans date tombe le jour de l'intervention :
                    // c'est le sens du `COALESCE`, pas un `NOW()` de dépannage
                    // qui la daterait du jour de la saisie.
                    tx.exec_drop(
                        "INSERT INTO llx_fichinterdet \
                         (fk_fichinter, fk_parent_line, special_code, product_type, \
                          date, description, duree, rang) \
                         VALUES (:fk, NULL, 0, 0, COALESCE(:date, :entete), :description, \
                          :duree, :rang)",
                        params! {
                            "fk" => rowid,
                            "date" => ligne.date.as_deref(),
                            "entete" => datei,
                            "description" => texte.as_deref(),
                            "duree" => ligne.duree,
                            "rang" => position as i64 + 1,
                        },
                    )?;
                }
                tx.commit()?;
                return Ok(rowid);
            }
            Err(e) if (is_duplicate(&e) || is_deadlock(&e)) && essai < 4 => {
                tx.rollback()?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("impossible d'attribuer une référence d'intervention unique".into())
}

/// Valide une intervention (`0 -> 1`) et date la validation.
///
/// Refus si elle est introuvable ou déjà validée : valider deux fois écraserait
/// la `date_valid` d'origine, qui est la date à laquelle le client a reçu le bon.
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_fichinter WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("intervention {rowid} introuvable").into()),
        Some(s) if s as i16 != STATUT_BROUILLON => {
            return Err(
                "cette intervention n'est pas en brouillon : elle est déjà validée ou clôturée"
                    .into(),
            )
        }
        _ => {}
    }
    conn.exec_drop(
        "UPDATE llx_fichinter SET fk_statut = :statut, date_valid = NOW() \
         WHERE entity = :entity AND rowid = :rowid",
        params! { "statut" => STATUT_VALIDE, "entity" => entity, "rowid" => rowid },
    )?;
    Ok(())
}

/// Clôt une intervention validée (`1 -> 2`) et pose sa date de fin.
///
/// Refus si elle n'est pas validée : une clôture sans validation donnerait une
/// intervention terminée mais jamais remise au client, donc invisible pour la
/// facturation.
pub fn cloturer(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    date_fin: &str,
) -> Result<(), Box<dyn Error>> {
    let date_fin = date_fin.trim();
    if date_fin.is_empty() {
        return Err("la date de fin d'intervention est obligatoire".into());
    }
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_fichinter WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("intervention {rowid} introuvable").into()),
        Some(s) if s as i16 != STATUT_VALIDE => {
            return Err("seule une intervention validée peut être clôturée".into())
        }
        _ => {}
    }
    conn.exec_drop(
        "UPDATE llx_fichinter SET fk_statut = :statut, datee = :datee \
         WHERE entity = :entity AND rowid = :rowid",
        params! {
            "statut" => STATUT_CLOTURE,
            "datee" => date_fin,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    Ok(())
}

/// Supprime une intervention en brouillon et ses lignes.
///
/// Refus dès qu'elle est validée (`fk_statut > 0`) : sa référence a pu être
/// imprimée, et la faire disparaître laisserait un trou dans la numérotation
/// sans trace de ce qui a été remis au client.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_fichinter WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    match statut {
        None => return Err(format!("intervention {rowid} introuvable").into()),
        Some(s) if s > i64::from(STATUT_BROUILLON) => {
            return Err("une intervention validée ne se supprime pas : elle se clôture".into())
        }
        _ => {}
    }
    let mut tx = conn.start_transaction(TxOpts::default())?;
    // Les lignes d'abord : `fk_fichinterdet_fk_fichinter` refuserait sinon.
    tx.exec_drop(
        "DELETE FROM llx_fichinterdet WHERE fk_fichinter = :rowid",
        params! { "rowid" => rowid },
    )?;
    tx.exec_drop(
        "DELETE FROM llx_fichinter WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    tx.commit()?;
    Ok(())
}

/// Temps passé, **en minutes** : durée de l'en-tête plus celle des lignes.
///
/// Les deux se cumulent parce qu'ils ne mesurent pas la même chose : l'en-tête
/// porte le déplacement — trajet, installation, temps qu'on ne détaille pas sur
/// un bon —, les lignes le travail tâche par tâche. Dolibarr les affiche
/// séparément ; ici on rend le total, et c'est à l'appelant de lire l'en-tête
/// seul s'il veut le détail (il l'a dans [`Fichinter::duree`]).
///
/// Une intervention sans ligne rend la durée de son en-tête ; une intervention
/// sans durée d'en-tête rend la somme de ses lignes ; les deux vides rendent
/// `0`, jamais une erreur : zéro minute est une information, pas une panne.
pub fn duree_totale(pool: &Pool, entity: i64, rowid: i64) -> Result<Decimal, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let row: Option<Row> = conn.exec_first(
        "SELECT CAST(COALESCE(f.duree, 0) AS CHAR), \
                CAST(COALESCE((SELECT SUM(d.duree) FROM llx_fichinterdet d \
                               WHERE d.fk_fichinter = f.rowid), 0) AS CHAR) \
         FROM llx_fichinter f WHERE f.entity = :entity AND f.rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(row) = row else {
        return Err(format!("intervention {rowid} introuvable").into());
    };
    let entete = opt_dec(&row, 0).unwrap_or_default();
    let lignes = opt_dec(&row, 1).unwrap_or_default();
    Ok(entete + lignes)
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1062)
}

fn is_deadlock(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(s) if s.code == 1213 || s.code == 1205)
}
