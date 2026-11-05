//! Dons (`llx_don`, `llx_payment_donation`).
//!
//! Un don, c'est un montant **promis** par un donateur, puis — le jour où
//! l'argent arrive — une ligne de règlement et un drapeau « encaissé ». Dolibarr
//! range l'en-tête dans `llx_don`, le règlement dans `llx_payment_donation`, et
//! relie les deux par `llx_don.fk_payment`.
//!
//! # Les deux drapeaux, et pourquoi ils ne disent pas la même chose
//!
//! Dolibarr porte deux informations distinctes sur un don :
//!
//! | colonne | sens | valeurs écrites ici |
//! |---|---|---|
//! | `fk_statut` | où en est le don dans le circuit | [`STATUT_PROMESSE`] à la création, [`STATUT_VALIDE`] après [`valider`] |
//! | `paid` | l'argent est-il arrivé ? | 0 à la création, 1 après [`encaisser`] |
//!
//! Le portage lit le statut 1 comme « promesse » et 2 comme « validé », parce
//! que c'est l'usage que ce noyau en fait : un don saisi n'est pas un don
//! contrôlé. [`encaisser`] ne touche **pas** `fk_statut` : l'arrivée de l'argent
//! et l'acte de validation sont deux gestes différents, et les confondre
//! effacerait la trace de l'un des deux. `paid`, lui, est le seul drapeau qui
//! dit la trésorerie.
//!
//! # Le don anonyme n'existe pas
//!
//! [`create`] refuse un don dont le nom **et** la société sont vides. Ce n'est
//! pas une coquetterie : un reçu, une relance de remerciement, une attestation
//! fiscale et un contrôle comptable ont tous besoin de savoir qui a donné.
//! Accepter une ligne sans personne produirait un montant rattaché à rien, que
//! personne ne pourrait plus rattacher. Un don au nom d'une association ou d'une
//! entreprise est en revanche parfaitement licite : c'est la société qui porte
//! alors l'identité, et le nom peut rester vide.
//!
//! # La référence
//!
//! `llx_don.ref` est un `varchar(30)` protégé par l'index `idx_don_uk_ref
//! (ref, entity)`. Ce module numérote avec [`divers::suivant_ref`]
//! (`DON0001`, `DON0002`…), comme les adhérents (`AD…`), les congés (`CP…`) et
//! les tickets (`TK…`) : une référence stable, lisible et qui tient dans la
//! colonne, là où un `rowid` inconnu avant l'insertion ne peut pas servir.
//!
//! # Ce que ce module ne fait pas
//!
//! - **Aucun reçu fiscal n'est produit** : `model_pdf` reste `NULL`, le PDF du
//!   don appartient à l'écran qui l'imprime.
//! - **Aucun tiers n'est créé ni modifié** : `fk_soc` reste `NULL`. Rapprocher
//!   un don d'une fiche société est une décision commerciale, pas une
//!   conséquence de la saisie ; `party` s'en charge quand l'écran le demande.
//! - **Aucune écriture comptable** : le don n'est pas ventilé en journal. Il
//!   n'existe pas de correspondance automatique entre `llx_payment_donation` et
//!   `llx_bank` dans le cœur de Dolibarr, et en inventer une fausserait le
//!   rapprochement bancaire.
//! - **Aucune suppression d'un don encaissé** : voir [`supprimer`].

use std::error::Error;
use std::str::FromStr;

use money::round_money;
use mysql::prelude::*;
use mysql::{params, Pool, Row, TxOpts};
use rust_decimal::Decimal;

use crate::divers;

/// Préfixe de référence des dons : `DON0001`, `DON0002`…
pub const PREFIX: &str = "DON";

/// Promesse : le don est enregistré, il n'a pas encore été contrôlé.
pub const STATUT_PROMESSE: i16 = 1;
/// Validé : le don a été contrôlé, il devient encaissable.
pub const STATUT_VALIDE: i16 = 2;

/// Un don tel qu'il se lit en base.
///
/// Le montant est un [`Decimal`] : `amount` est un `double(24,8)` en base, mais
/// il sort en `CAST(... AS CHAR)` et ne transite jamais par un flottant.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Don {
    pub rowid: i64,
    pub entity: i64,
    /// Référence du don (`DON0001`).
    pub ref_: String,
    /// `fk_statut` : [`STATUT_PROMESSE`] ou [`STATUT_VALIDE`].
    pub statut: i16,
    /// `datedon`, au format `AAAA-MM-JJ`. Vaut le jour de la saisie quand
    /// l'appelant n'a pas fourni de date.
    pub datedon: Option<String>,
    /// Montant promis.
    pub montant: Decimal,
    /// Ligne de `llx_payment_donation` qui a soldé le don, si elle existe.
    pub fk_payment: Option<i64>,
    /// `paid` : vrai quand l'argent est arrivé.
    pub encaisse: bool,
    pub fk_soc: Option<i64>,
    pub prenom: Option<String>,
    /// `lastname` : facultatif, un don peut n'être porté que par une société.
    pub nom: Option<String>,
    pub societe: Option<String>,
    pub adresse: Option<String>,
    pub cp: Option<String>,
    pub ville: Option<String>,
    /// `fk_country` : identifiant du dictionnaire `llx_c_country`, 0 si absent.
    pub pays: i64,
    pub email: Option<String>,
    pub telephone: Option<String>,
    pub portable: Option<String>,
    /// `public` : le don peut-il être publié ? Faux par défaut, voir [`create`].
    pub public: bool,
    pub fk_projet: Option<i64>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    pub date_creation: Option<String>,
    /// `date_valid` : moment de la validation.
    pub date_validation: Option<String>,
}

impl Don {
    /// Le nom du donateur, tel qu'on l'écrit dans un remerciement.
    ///
    /// « Prénom Nom », complété de la société entre parenthèses quand les deux
    /// existent, « Société » seule quand le nom est vide. À défaut de tout, la
    /// référence du don : une chaîne vide dans un courrier serait pire qu'une
    /// référence, qui au moins désigne la bonne ligne.
    pub fn donateur(&self) -> String {
        let prenom = self.prenom.as_deref().map(str::trim).unwrap_or("");
        let nom = self.nom.as_deref().map(str::trim).unwrap_or("");
        let personne = match (prenom.is_empty(), nom.is_empty()) {
            (false, false) => format!("{prenom} {nom}"),
            (true, false) => nom.to_string(),
            (false, true) => prenom.to_string(),
            (true, true) => String::new(),
        };
        let societe = self.societe.as_deref().map(str::trim).unwrap_or("");
        match (personne.is_empty(), societe.is_empty()) {
            (false, false) => format!("{personne} ({societe})"),
            (false, true) => personne,
            (true, false) => societe.to_string(),
            (true, true) => self.ref_.clone(),
        }
    }

    /// Vrai quand le don peut encore être encaissé : validé et pas déjà payé.
    pub fn encaissable(&self) -> bool {
        !self.encaisse && self.fk_payment.is_none() && self.statut >= STATUT_VALIDE
    }
}

/// Ce qu'il faut pour créer un don.
///
/// Les champs texte facultatifs acceptent `None` ; une chaîne vide est rangée
/// en `NULL`, pour ne pas faire croire à une saisie. `fk_country` vaut 0 quand
/// l'appelant ne le renseigne pas — la colonne est NOT NULL et Dolibarr accepte
/// 0 comme « pays non précisé ».
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewDon {
    /// `lastname` — peut rester vide si `societe` est renseignée.
    pub lastname: String,
    pub firstname: Option<String>,
    pub societe: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub zip: Option<String>,
    pub town: Option<String>,
    /// `fk_country` ; 0 si non fourni.
    pub fk_country: i64,
    /// Montant promis, strictement positif.
    pub amount: Decimal,
    /// `datedon` (`AAAA-MM-JJ`) ; `None` = le jour de la saisie.
    pub datedon: Option<String>,
    pub fk_projet: Option<i64>,
    pub note_private: Option<String>,
    pub note_public: Option<String>,
    /// Auteur de la saisie (`llx_user.rowid`), strictement positif.
    pub fk_user_author: i64,
}

const SELECT_COLS: &str = "d.rowid, d.entity, d.ref, d.fk_statut, \
     DATE_FORMAT(d.datedon, '%Y-%m-%d'), CAST(d.amount AS CHAR), d.fk_payment, d.paid, \
     d.fk_soc, d.firstname, d.lastname, d.societe, d.address, d.zip, d.town, \
     COALESCE(d.fk_country, 0), d.email, d.phone, d.phone_mobile, d.public, d.fk_projet, \
     d.note_private, d.note_public, \
     DATE_FORMAT(d.datec, '%Y-%m-%d %H:%i:%s'), \
     DATE_FORMAT(d.date_valid, '%Y-%m-%d %H:%i:%s')";

fn opt_s(row: &Row, i: usize) -> Option<String> {
    row.get_opt::<String, _>(i).and_then(|r| r.ok())
}

fn opt_i(row: &Row, i: usize) -> Option<i64> {
    row.get_opt::<i64, _>(i).and_then(|r| r.ok())
}

fn opt_dec(row: &Row, i: usize) -> Option<Decimal> {
    opt_s(row, i).and_then(|s| Decimal::from_str(s.trim()).ok())
}

fn map_row(row: &Row) -> Don {
    Don {
        rowid: opt_i(row, 0).unwrap_or(0),
        entity: opt_i(row, 1).unwrap_or(1),
        ref_: opt_s(row, 2).unwrap_or_default(),
        statut: opt_i(row, 3).unwrap_or(0) as i16,
        datedon: opt_s(row, 4),
        montant: opt_dec(row, 5).unwrap_or_default(),
        fk_payment: opt_i(row, 6),
        encaisse: opt_i(row, 7).unwrap_or(0) != 0,
        fk_soc: opt_i(row, 8),
        prenom: opt_s(row, 9),
        nom: opt_s(row, 10),
        societe: opt_s(row, 11),
        adresse: opt_s(row, 12),
        cp: opt_s(row, 13),
        ville: opt_s(row, 14),
        pays: opt_i(row, 15).unwrap_or(0),
        email: opt_s(row, 16),
        telephone: opt_s(row, 17),
        portable: opt_s(row, 18),
        public: opt_i(row, 19).unwrap_or(0) != 0,
        fk_projet: opt_i(row, 20),
        note_private: opt_s(row, 21),
        note_public: opt_s(row, 22),
        date_creation: opt_s(row, 23),
        date_validation: opt_s(row, 24),
    }
}

/// Un champ texte facultatif dont la chaîne vide vaut « pas de valeur ».
fn texte(valeur: Option<&str>) -> Option<&str> {
    valeur.map(str::trim).filter(|v| !v.is_empty())
}

fn is_duplicate(e: &mysql::Error) -> bool {
    matches!(e, mysql::Error::MySqlError(me) if me.code == 1062)
}

/// Une date `AAAA-MM-JJ` réelle : `2027-02-31` a le bon format mais n'existe
/// pas au calendrier, et MySQL refuserait l'écriture au milieu de la mise à
/// jour, avec un message qui ne dit pas quel champ est en cause.
fn date_valide(date: &str) -> bool {
    let octets = date.as_bytes();
    if octets.len() != 10 || octets[4] != b'-' || octets[7] != b'-' {
        return false;
    }
    if !octets
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    // Tous les octets restants sont des chiffres : la soustraction est sûre.
    let nombre = |debut: usize, fin: usize| {
        octets[debut..fin]
            .iter()
            .fold(0u32, |cumul, c| cumul * 10 + u32::from(c - b'0'))
    };
    let (annee, mois, jour) = (nombre(0, 4), nombre(5, 7), nombre(8, 10));
    annee >= 1 && jour >= 1 && jour <= jours_dans_le_mois(annee, mois)
}

/// Nombre de jours d'un mois, `0` si le mois n'existe pas.
fn jours_dans_le_mois(annee: u32, mois: u32) -> u32 {
    match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if annee.is_multiple_of(400)
            || (annee.is_multiple_of(4) && !annee.is_multiple_of(100)) =>
        {
            29
        }
        2 => 28,
        _ => 0,
    }
}

/// Une heure `HH:MM` ou `HH:MM:SS`.
fn heure_valide(heure: &str) -> bool {
    let parties: Vec<&str> = heure.split(':').collect();
    if parties.len() != 2 && parties.len() != 3 {
        return false;
    }
    if !parties
        .iter()
        .all(|p| p.len() == 2 && p.bytes().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    let lire = |p: &str| p.parse::<u32>().unwrap_or(u32::MAX);
    let secondes_ok = parties.len() == 2 || lire(parties[2]) < 60;
    lire(parties[0]) < 24 && lire(parties[1]) < 60 && secondes_ok
}

/// Une date seule (`AAAA-MM-JJ`) ou une date et une heure
/// (`AAAA-MM-JJ HH:MM[:SS]`), les deux formes que MySQL accepte pour un
/// `datetime`.
fn datep_valide(valeur: &str) -> bool {
    match valeur.split_once(' ') {
        None => date_valide(valeur),
        Some((jour, heure)) => date_valide(jour) && heure_valide(heure),
    }
}

/// Crée un don et renvoie son `rowid`.
///
/// La ligne est écrite au statut [`STATUT_PROMESSE`], `paid = 0`, et
/// `datec = NOW()`. `fk_user_author` est le seul utilisateur nommé : le noyau
/// n'ouvre pas de session, il ne peut donc pas signer une validation
/// (`fk_user_valid` reste `NULL`) — c'est [`valider`] qui date l'acte.
///
/// `public` est écrit à **0**. Le schéma pose `DEFAULT 1`, mais publier le nom
/// d'un donateur sans son accord serait une fuite : la valeur par défaut du
/// formulaire n'est pas un consentement. L'écran qui recueille l'accord passera
/// par une écriture explicite.
///
/// # Erreurs
///
/// Nom **et** société vides, montant nul ou négatif, auteur absent
/// (`fk_user_author <= 0`), email fourni sans arobase, date de don qui n'existe
/// pas au calendrier, référence déjà attribuée, et toute erreur MySQL remontée
/// telle quelle.
pub fn create(pool: &Pool, entity: i64, new: &NewDon) -> Result<i64, Box<dyn Error>> {
    let nom = new.lastname.trim();
    let societe = texte(new.societe.as_deref());
    if nom.is_empty() && societe.is_none() {
        return Err(
            "un don doit porter un nom ou une société : sans l'un des deux, \
             on ne sait pas qui a donné"
                .into(),
        );
    }

    let montant = round_money(new.amount);
    if montant <= Decimal::ZERO {
        return Err(
            format!("le montant du don doit être strictement positif (reçu : {montant})").into(),
        );
    }

    if new.fk_user_author <= 0 {
        return Err("l'auteur du don (fk_user_author) est obligatoire".into());
    }

    // Même garde que pour un adhérent : un email sans arobase est inutilisable
    // pour remercier le donateur, et Dolibarr ne s'en apercevrait qu'à l'envoi.
    let email = texte(new.email.as_deref());
    if let Some(saisi) = email.filter(|e| !e.contains('@')) {
        return Err(format!("l'adresse email « {saisi} » ne contient pas d'arobase").into());
    }

    let datedon = texte(new.datedon.as_deref());
    if let Some(jour) = datedon {
        if !date_valide(jour) {
            return Err(
                format!("la date du don « {jour} » n'est pas une date AAAA-MM-JJ valide").into(),
            );
        }
    }

    let reference = divers::suivant_ref(pool, "llx_don", "ref", PREFIX, entity)?;
    let mut conn = pool.get_conn()?;
    let insere = conn.exec_drop(
        // `datedon` absent : `CURDATE()`, comme le formulaire Dolibarr qui
        // pré-remplit la date du jour. Un don sans date serait inexploitable
        // dans les statistiques, alors qu'il est toujours saisi un jour précis.
        "INSERT INTO llx_don \
         (entity, ref, fk_statut, datedon, amount, paid, firstname, lastname, \
          societe, address, zip, town, fk_country, email, public, fk_projet, datec, \
          fk_user_author, note_private, note_public) \
         VALUES \
         (:entity, :ref, :statut, COALESCE(:datedon, CURDATE()), :montant, 0, :prenom, :nom, \
          :societe, :adresse, :cp, :ville, :pays, :email, :public, :projet, NOW(), \
          :auteur, :note_private, :note_public)",
        params! {
            "entity" => entity,
            "ref" => reference.as_str(),
            "statut" => STATUT_PROMESSE,
            "datedon" => datedon,
            "montant" => montant.to_string(),
            "prenom" => texte(new.firstname.as_deref()),
            "nom" => texte(Some(nom)),
            "societe" => societe,
            "adresse" => texte(new.address.as_deref()),
            "cp" => texte(new.zip.as_deref()),
            "ville" => texte(new.town.as_deref()),
            "pays" => new.fk_country,
            "email" => email,
            "public" => 0i16,
            "projet" => new.fk_projet,
            "auteur" => new.fk_user_author,
            "note_private" => texte(new.note_private.as_deref()),
            "note_public" => texte(new.note_public.as_deref()),
        },
    );

    // Pas de boucle de reprise : `divers::suivant_ref` recalcule `MAX(ref) + 1`
    // et rendrait le même numéro au second essai. Rejouer ne servirait à rien.
    match insere {
        Ok(()) => Ok(conn.last_insert_id() as i64),
        Err(e) if is_duplicate(&e) => Err(format!(
            "la référence « {reference} » vient d'être prise par une autre création : réessayez"
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

/// Les dons d'une entité, du plus récent au plus ancien.
///
/// L'ordre suit la date du don, puis `rowid` : deux dons saisis dans la même
/// seconde gardent un ordre stable. Un don sans `datedon` (ligne fabriquée à la
/// main) retombe sur sa date de création.
pub fn list(
    pool: &Pool,
    entity: i64,
    limite: u32,
    offset: u32,
) -> Result<Vec<Don>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_don d \
         WHERE d.entity = :entity \
         ORDER BY COALESCE(d.datedon, d.datec) DESC, d.rowid DESC LIMIT :limite OFFSET :offset"
    );
    let rows: Vec<Row> = conn.exec(
        sql,
        params! { "entity" => entity, "limite" => limite, "offset" => offset },
    )?;
    Ok(rows.iter().map(map_row).collect())
}

/// Un don précis, ou `None` s'il n'appartient pas à cette entité.
pub fn get(pool: &Pool, entity: i64, rowid: i64) -> Result<Option<Don>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM llx_don d \
         WHERE d.entity = :entity AND d.rowid = :rowid"
    );
    let row: Option<Row> =
        conn.exec_first(sql, params! { "entity" => entity, "rowid" => rowid })?;
    Ok(row.as_ref().map(map_row))
}

/// Valide un don : `fk_statut` passe à [`STATUT_VALIDE`] et `date_valid` est
/// posée.
///
/// Un don au statut 0 (brouillon écrit par Dolibarr) est validé d'un coup : le
/// circuit de Dolibarr prévoit deux étapes, ce noyau n'en a qu'une et refuse de
/// laisser une ligne bloquée dans un état que rien ici ne ferait avancer.
///
/// # Erreurs
///
/// Don introuvable dans l'entité, don **déjà validé** (statut ≥ 2), ou don
/// validé entre-temps par un autre appel (l'`UPDATE` conditionnel ne touche
/// alors aucune ligne).
pub fn valider(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let statut: Option<i64> = conn.exec_first(
        "SELECT fk_statut FROM llx_don WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(statut) = statut else {
        return Err(format!("le don {rowid} n'existe pas dans l'entité {entity}").into());
    };
    if statut >= i64::from(STATUT_VALIDE) {
        return Err(format!("le don {rowid} est déjà validé : rien à faire").into());
    }

    conn.exec_drop(
        // La condition `fk_statut < :avant` fait le tri en cas de double clic :
        // le second appel ne touche aucune ligne.
        "UPDATE llx_don SET fk_statut = :statut, date_valid = NOW(), tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid AND fk_statut < :avant",
        params! {
            "statut" => STATUT_VALIDE,
            "avant" => STATUT_VALIDE,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    if conn.affected_rows() == 0 {
        return Err(format!(
            "le don {rowid} vient d'être validé par un autre appel : rechargez la fiche"
        )
        .into());
    }
    Ok(())
}

/// Enregistre l'encaissement d'un don et renvoie l'identifiant du règlement.
///
/// Le montant du règlement est **recopié du don** : on n'encaisse pas autre
/// chose que ce qui a été promis, un écart serait une écriture comptable que
/// personne n'a décidée. La ligne va dans `llx_payment_donation` (table de
/// paiement séparée, comme `llx_payment_loan` pour les prêts), puis
/// `llx_don.paid` passe à 1 et `fk_payment` pointe dessus.
///
/// `date` accepte `AAAA-MM-JJ` ou `AAAA-MM-JJ HH:MM:SS`. Une date vide est
/// refusée : un encaissement sans date ne se rapproche de rien.
///
/// # Refus, et pourquoi
///
/// 1. **Don non validé** : encaisser une simple promesse ferait entrer en
///    trésorerie un montant que personne n'a contrôlé.
/// 2. **Don déjà encaissé** (`paid = 1` ou `fk_payment` renseigné) : deux
///    règlements pour un même don compteraient deux fois le même argent. Le
///    refus ne repose pas sur la lecture préalable mais sur l'`UPDATE`
///    conditionnel `... AND paid = 0 AND fk_payment IS NULL`, donc deux appels
///    simultanés ne peuvent pas passer tous les deux.
/// 3. **Compte bancaire inconnu** de l'entité : `fk_bank` est NOT NULL, et un
///    identifiant fantôme rendrait le rapprochement impossible.
/// 4. **Moyen de paiement inconnu** du dictionnaire `llx_c_paiement`.
///
/// # Erreurs
///
/// Les quatre refus ci-dessus, une date mal formée, un don introuvable, et
/// toute erreur MySQL remontée telle quelle.
pub fn encaisser(
    pool: &Pool,
    entity: i64,
    rowid: i64,
    fk_bank: i64,
    fk_typepayment: i64,
    date: &str,
) -> Result<i64, Box<dyn Error>> {
    if fk_typepayment <= 0 {
        return Err("le moyen de paiement (fk_typepayment) est obligatoire".into());
    }
    let datep = date.trim();
    if !datep_valide(datep) {
        return Err(format!(
            "la date d'encaissement « {date} » n'est pas une date AAAA-MM-JJ valide"
        )
        .into());
    }

    let mut conn = pool.get_conn()?;
    let ligne: Option<Row> = conn.exec_first(
        "SELECT CAST(d.amount AS CHAR), d.paid, d.fk_payment, d.fk_statut \
         FROM llx_don d WHERE d.entity = :entity AND d.rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some(ligne) = ligne else {
        return Err(format!("le don {rowid} n'existe pas dans l'entité {entity}").into());
    };

    let montant = round_money(opt_dec(&ligne, 0).unwrap_or_default());
    let paid = opt_i(&ligne, 1).unwrap_or(0);
    let fk_payment = opt_i(&ligne, 2);
    let statut = opt_i(&ligne, 3).unwrap_or(0) as i16;

    if paid != 0 || fk_payment.is_some() {
        let deja = match fk_payment {
            Some(p) => format!(" (paiement {p})"),
            None => String::new(),
        };
        return Err(format!("le don {rowid} est déjà encaissé{deja}").into());
    }
    if statut < STATUT_VALIDE {
        return Err(
            format!("le don {rowid} n'est pas validé : validez-le avant de l'encaisser").into(),
        );
    }
    if montant <= Decimal::ZERO {
        return Err(format!(
            "le don {rowid} porte un montant nul ou négatif ({montant}) : rien à encaisser"
        )
        .into());
    }

    if !compte_existe(&mut conn, entity, fk_bank)? {
        return Err(
            format!("le compte bancaire {fk_bank} n'existe pas dans l'entité {entity}").into(),
        );
    }
    if !moyen_existe(&mut conn, fk_typepayment)? {
        return Err(format!(
            "le moyen de paiement {fk_typepayment} n'existe pas dans le dictionnaire llx_c_paiement"
        )
        .into());
    }

    let mut tx = conn.start_transaction(TxOpts::default())?;
    tx.exec_drop(
        // `fk_user_creat` reste NULL : aucune session ouverte, et inventer un
        // utilisateur serait pire que l'absence.
        "INSERT INTO llx_payment_donation \
         (fk_donation, datec, datep, amount, fk_typepayment, num_payment, note, fk_bank, \
          fk_user_creat) \
         VALUES (:don, NOW(), :datep, :montant, :moyen, NULL, NULL, :banque, NULL)",
        params! {
            "don" => rowid,
            "datep" => datep,
            "montant" => montant.to_string(),
            "moyen" => fk_typepayment,
            "banque" => fk_bank,
        },
    )?;
    let paiement = tx.last_insert_id().unwrap_or(0) as i64;

    tx.exec_drop(
        "UPDATE llx_don SET paid = 1, fk_payment = :paiement, tms = NOW() \
         WHERE entity = :entity AND rowid = :rowid AND paid = 0 AND fk_payment IS NULL",
        params! {
            "paiement" => paiement,
            "entity" => entity,
            "rowid" => rowid,
        },
    )?;
    if tx.affected_rows() == 0 {
        tx.rollback()?;
        return Err(format!("le don {rowid} vient d'être encaissé par un autre appel").into());
    }
    tx.commit()?;
    Ok(paiement)
}

/// Supprime un don **non encaissé**.
///
/// Un don dont l'argent est arrivé ne se supprime pas : il justifie une entrée
/// de trésorerie, et l'effacer laisserait un règlement orphelin dans
/// `llx_payment_donation`. Le geste correct est d'annuler l'encaissement
/// depuis Dolibarr, puis de supprimer.
///
/// # Erreurs
///
/// Don introuvable dans l'entité, don encaissé (`paid = 1`), don relié à un
/// règlement (`fk_payment` renseigné) même si `paid` a été remis à 0 à la main.
pub fn supprimer(pool: &Pool, entity: i64, rowid: i64) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ligne: Option<(i64, Option<i64>)> = conn.exec_first(
        "SELECT paid, fk_payment FROM llx_don WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    let Some((paid, fk_payment)) = ligne else {
        return Err(format!("le don {rowid} n'existe pas dans l'entité {entity}").into());
    };
    if paid != 0 {
        return Err(format!(
            "le don {rowid} est encaissé : il ne se supprime pas, annulez d'abord l'encaissement"
        )
        .into());
    }
    if let Some(paiement) = fk_payment {
        return Err(format!(
            "le don {rowid} est relié au règlement {paiement} : supprimez le règlement d'abord"
        )
        .into());
    }

    conn.exec_drop(
        "DELETE FROM llx_don WHERE entity = :entity AND rowid = :rowid AND paid = 0",
        params! { "entity" => entity, "rowid" => rowid },
    )?;
    if conn.affected_rows() == 0 {
        return Err(format!(
            "le don {rowid} vient d'être encaissé par un autre appel : rien n'a été supprimé"
        )
        .into());
    }
    Ok(())
}

/// Totaux d'une entité : `(promis, encaissé)`.
///
/// Le premier terme est la somme de **tous** les dons de l'entité — ce que les
/// donateurs ont promis, encaissé compris —, le second la part effectivement
/// encaissée. Un tableau de bord les lit ensemble : « 1 500 € promis, dont
/// 900 € encaissés ». Les deux valeurs sont arrondies au centime, une seule
/// fois, à la sortie.
///
/// Une entité sans aucun don rend `(0, 0)` : l'absence de don n'est pas une
/// erreur, c'est un résultat.
pub fn total(pool: &Pool, entity: i64) -> Result<(Decimal, Decimal), Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let ligne: Option<(Option<String>, Option<String>)> = conn.exec_first(
        "SELECT CAST(COALESCE(SUM(d.amount), 0) AS CHAR), \
                CAST(COALESCE(SUM(CASE WHEN d.paid = 1 THEN d.amount ELSE 0 END), 0) AS CHAR) \
         FROM llx_don d WHERE d.entity = :entity",
        params! { "entity" => entity },
    )?;
    let (promis, encaisse) = ligne.unwrap_or((None, None));
    Ok((
        round_money(lire_dec(promis)),
        round_money(lire_dec(encaisse)),
    ))
}

/// Un montant lu en texte, `0` si la colonne est vide ou illisible.
fn lire_dec(valeur: Option<String>) -> Decimal {
    valeur
        .and_then(|s| Decimal::from_str(s.trim()).ok())
        .unwrap_or_default()
}

/// Le compte bancaire existe-t-il dans cette entité ?
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

/// Le moyen de paiement existe-t-il dans le dictionnaire ?
///
/// `llx_c_paiement` est un dictionnaire global : les lignes ne portent pas
/// l'entité de la société, d'où l'absence de filtre sur `entity`. L'état
/// `active` n'est pas vérifié : un mode désactivé reste un mode réel, et
/// refuser un encaissement déjà décidé pour cette raison bloquerait la
/// trésorerie sans rien protéger.
fn moyen_existe<Q>(conn: &mut Q, fk_typepayment: i64) -> Result<bool, Box<dyn Error>>
where
    Q: Queryable,
{
    let existe: Option<i64> = conn.exec_first(
        "SELECT id FROM llx_c_paiement WHERE id = :moyen",
        params! { "moyen" => fk_typepayment },
    )?;
    Ok(existe.is_some())
}
