//! Ecritures tenues : `crate::ecritures` produit l'ecriture, ce module la met
//! en base dans **la vraie table de Dolibarr**, `llx_accounting_bookkeeping`.
//!
//! Le grand livre n'est plus seulement derive : les lignes existent, Dolibarr
//! PHP les lit comme les siennes, et l'export FEC porte sur une comptabilite
//! reellement tenue.
//!
//! # Colonnes ecrites (et rien d'autre)
//!
//! Relevees dans `schema-24.0.1.json` (`tables.llx_accounting_bookkeeping`) et
//! verifiees sur `information_schema` de la base 24.0.1 :
//!
//! | colonne | type | valeur ecrite |
//! |---|---|---|
//! | `entity` | `int NOT NULL` | l'entite demandee (cloisonnement) |
//! | `ref` | `varchar(30) NULL` | le **numero d'ecriture** (`VE-FA2611-0015`) |
//! | `piece_num` | `int NOT NULL` | numero de piece : `MAX(piece_num)+1` du couple (entite, journal), **le meme pour toutes les lignes d'une ecriture** |
//! | `doc_date` | `date NOT NULL` | `Ecriture::date`, refusee si elle n'est pas `AAAA-MM-JJ` |
//! | `doc_type` | `varchar(30) NOT NULL` | [`DOC_TYPE`] : marque l'origine des lignes |
//! | `doc_ref` | `varchar(300) NOT NULL` | reference de la piece, tiree du numero (`FA2611-0015`) |
//! | `fk_doc`, `fk_docdet` | `int NOT NULL` | `0` : l'ecriture ne porte pas l'identifiant du document, seulement sa reference |
//! | `numero_compte` | `varchar(32) NOT NULL` | `Ligne::compte` |
//! | `label_compte` | `varchar(255) NOT NULL` | libelle du compte (table de `crate::grand_livre`) |
//! | `label_operation` | `varchar(255) NULL` | `Ligne::libelle` |
//! | `debit` | `double(24,8) NOT NULL` | montant au debit, **`0` si la ligne est au credit** |
//! | `credit` | `double(24,8) NOT NULL` | montant au credit, **`0` si la ligne est au debit** |
//! | `montant` | `double(24,8) NULL` | **valeur absolue** du montant de la ligne |
//! | `sens` | `varchar(1) NULL` | `D` au debit, `C` au credit |
//! | `code_journal` | `varchar(32) NOT NULL` | `Ligne::journal` (`VE`, `AC`) |
//! | `journal_label` | `varchar(255) NULL` | libelle du journal, celui de `crate::fec::JOURNAUX` |
//! | `fk_user_author` | `int NOT NULL` | `0` : saisie par le noyau, pas par un utilisateur Dolibarr |
//! | `date_creation` | `datetime NULL` | `NOW()` |
//! | `import_key` | `varchar(14) NULL` | marqueur de ligne, voir plus bas |
//!
//! Ne sont pas ecrites : `thirdparty_code`, `subledger_account`, `subledger_label`
//! (le pont ne porte pas de tiers comptable), `multicurrency_amount` /
//! `multicurrency_code` (tout est en euros), `lettering_code` /
//! `date_lettering` (aucun lettrage), `date_validated` et `date_export` (une
//! ligne ecrite ici n'est **pas** validee au sens de Dolibarr : elle reste
//! modifiable dans l'interface), `matching_general`, `extraparams`.
//!
//! # Conventions Dolibarr respectees
//!
//! - **un seul sens par ligne** : `debit` **ou** `credit`, l'autre vaut `0` ;
//!   `comptabilite::verifier` le garantit deja, la base l'exige aussi ;
//! - `montant` porte la **valeur absolue** et `sens` vaut `D` ou `C` : c'est la
//!   convention de `BookKeeping::create()` (Dolibarr n'ecrit jamais un montant
//!   negatif, le sens dit le cote) ;
//! - `piece_num` est `NOT NULL` et **regroupe** les lignes d'une ecriture :
//!   toutes les lignes ecrites ici partagent le meme numero, pris a la suite du
//!   maximum du journal dans l'entite ;
//! - `fk_user_author` est `NOT NULL` : `0` (aucun utilisateur Dolibarr n'est
//!   l'auteur).
//!
//! # Idempotence : `DELETE` cible puis `INSERT`
//!
//! `llx_accounting_bookkeeping` n'a **aucun index unique** (`SHOW CREATE TABLE`
//! : sept index, tous non uniques ; `import_key` n'est pas indexe du tout). Un
//! `INSERT ... ON DUPLICATE KEY UPDATE` ne se declencherait donc **jamais** : il
//! creerait un doublon a chaque passage. On supprime d'abord, on insere ensuite,
//! dans une transaction :
//!
//! ```sql
//! DELETE FROM llx_accounting_bookkeeping WHERE entity = :entity AND ref = :numero
//! ```
//!
//! La cible est exacte : `ref` porte le numero d'ecriture, qui identifie
//! l'ecriture (`db::ecritures` le rend stable : `VE-<reference>`). Aucune autre
//! ligne n'est touchee - ni celles de Dolibarr (autre `ref`), ni celles de
//! `crate::fec` (qui ne renseigne pas `ref`).
//!
//! `import_key` recoit malgre tout une valeur stable, `ZZ-ECR-` suivi de sept
//! caracteres de base 36 : l'empreinte FNV-1a de (numero, compte, libelle,
//! debit, credit). La colonne fait `varchar(14)` - la taille d'un horodatage
//! `AAAAMMJJHHMMSS`, la valeur que Dolibarr y met - donc la cle entiere n'y
//! tient pas. C'est un **marqueur**, pas l'identite : une collision entre deux
//! lignes serait sans consequence puisqu'on ne supprime jamais par cette
//! colonne. Le prefixe `ZZ-ECR-` est aussi celui que les tests d'integration
//! utilisent pour nettoyer (`import_key LIKE 'ZZ-ECR-%'`).
//!
//! # Ce que ce module ne fait pas
//!
//! - **aucun lettrage** : `lettering_code`, `date_lettering` et
//!   `matching_general` restent vierges ; le rapprochement creance/encaissement
//!   reste a la charge de l'appelant ;
//! - **aucune cloture**, aucun exercice, aucun a-nouveau, aucune extourne ;
//! - **aucune validation Dolibarr** : `date_validated` reste `NULL` ;
//! - **aucune ecriture de banque** : ce module ecrit ce que `crate::ecritures`
//!   produit, c'est-a-dire `VE` et `AC`. Les journaux `BQ`, `OD` et `AN` ne
//!   passent pas par ici ;
//! - **aucune autre table** : ni `llx_facture`, ni `llx_societe`, ni aucun
//!   dictionnaire. Uniquement `llx_accounting_bookkeeping` ;
//! - **aucune coexistence avec `crate::fec::generer`** : les deux ecrivent la
//!   meme periode dans la meme table. `fec::generer` efface puis reinsere les
//!   journaux `VE`/`AC`/`BQ` de la periode sans regarder `doc_type` ; lancer les
//!   deux sur la meme periode double donc les lignes. Ce module ne peut pas
//!   l'empecher depuis son propre fichier : c'est a l'appelant de choisir son
//!   ecrivain.

use std::error::Error;

use comptabilite::{Ecriture, Ligne};
use mysql::prelude::*;
use mysql::{params, IsolationLevel, Pool, TxOpts};
use rust_decimal::Decimal;

/// Valeur de `doc_type` : marque les lignes ecrites par ce module.
///
/// `varchar(30)`, distincte de `dolibarr_rs` (que pose `crate::fec`) et des
/// valeurs de Dolibarr (`facture`, `facture_fourn`, ...), pour qu'on sache
/// toujours qui a ecrit une ligne.
pub const DOC_TYPE: &str = "dolibarr_rs_ecriture";

/// Prefixe de `import_key` : marqueur des lignes de ce module, et motif de
/// nettoyage des tests d'integration.
pub const PREFIXE_CLE: &str = "ZZ-ECR-";

/// Longueur de `llx_accounting_bookkeeping.import_key`.
const TAILLE_IMPORT_KEY: usize = 14;

/// Longueur de `llx_accounting_bookkeeping.ref`.
const TAILLE_REF: usize = 30;

/// Empreinte FNV-1a 64 bits : stable d'une compilation a l'autre, sans
/// dependance supplementaire.
fn empreinte(texte: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for octet in texte.as_bytes() {
        h ^= u64::from(*octet);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// `n` en base 36, sur exactement `largeur` caracteres, zero en tete.
fn base36(mut n: u64, largeur: usize) -> String {
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut chiffres = vec![b'0'; largeur];
    for chiffre in chiffres.iter_mut().rev() {
        *chiffre = ALPHABET[(n % 36) as usize];
        n /= 36;
    }
    chiffres.into_iter().map(char::from).collect()
}

/// `import_key` d'une ligne : marqueur stable, jamais une identite.
///
/// Voir l'en-tete du module : la colonne ne fait que 14 caracteres, on ne peut
/// y mettre qu'une empreinte. Ce qui identifie une ecriture en base, c'est
/// `ref` (le numero), pas cette colonne.
fn cle_import_key(numero: &str, ligne: &Ligne) -> String {
    let identite = format!(
        "{}|{}|{}|{}|{}",
        numero.trim(),
        ligne.compte.trim(),
        ligne.libelle.trim(),
        ligne.debit,
        ligne.credit
    );
    let taille = TAILLE_IMPORT_KEY - PREFIXE_CLE.len();
    format!("{PREFIXE_CLE}{}", base36(empreinte(&identite), taille))
}

/// Libelle d'un compte, celui de `crate::grand_livre`.
///
/// La fonction de `grand_livre` est privee : les deux livres doivent nommer un
/// compte de la meme facon, sinon la balance et le grand livre ne se relisent
/// plus pareil. Toute valeur ajoutee la-bas doit l'etre ici.
fn libelle_compte(compte: &str) -> &'static str {
    match compte {
        crate::ecritures::COMPTE_CLIENTS => "Clients",
        crate::ecritures::COMPTE_FOURNISSEURS => "Fournisseurs",
        "512000" => "Banque",
        crate::ecritures::COMPTE_VENTES => "Prestations de services",
        crate::ecritures::COMPTE_ACHATS => "Achats",
        crate::ecritures::COMPTE_TVA_COLLECTEE => "TVA collectee",
        crate::ecritures::COMPTE_TVA_DEDUCTIBLE => "TVA deductible",
        _ => "Compte",
    }
}

/// Libelle d'un journal, celui de `crate::fec::JOURNAUX`.
fn libelle_journal(code: &str) -> &'static str {
    crate::fec::JOURNAUX
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, l)| *l)
        .unwrap_or("Journal")
}

/// Refuse une valeur plus longue que la colonne : jamais de troncature
/// silencieuse, une ligne de grand livre ne se coupe pas.
fn borner(valeur: &str, max: usize, quoi: &str) -> Result<(), Box<dyn Error>> {
    let longueur = valeur.chars().count();
    if longueur > max {
        return Err(format!(
            "{quoi} : {longueur} caracteres pour un maximum de {max} (\"{valeur}\")"
        )
        .into());
    }
    Ok(())
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

/// Une date `AAAA-MM-JJ` reelle : la colonne `doc_date` est un `date`, et un
/// `2026-02-31` refuse par le serveur ferait echouer la transaction sur une
/// erreur MySQL au lieu de dire ce qui ne va pas.
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
    // Les octets sont tous des chiffres a ce stade : la soustraction est sure.
    let nombre = |debut: usize, fin: usize| {
        octets[debut..fin]
            .iter()
            .fold(0u32, |cumul, c| cumul * 10 + u32::from(c - b'0'))
    };
    let (annee, mois, jour) = (nombre(0, 4), nombre(5, 7), nombre(8, 10));
    annee >= 1 && jour >= 1 && jour <= jours_dans_le_mois(annee, mois)
}

/// Reference de la piece justificative, tiree du numero d'ecriture.
///
/// `crate::ecritures` numerote `<journal>-<reference du document>` : le journal
/// est ce qui precede le premier tiret, la piece est le reste. Un numero qui
/// n'a pas cette forme est refuse - sans reference de piece, une ligne de grand
/// livre ne se justifie pas.
fn reference_piece(numero: &str) -> Result<String, Box<dyn Error>> {
    let Some((journal, piece)) = numero.split_once('-') else {
        return Err(format!(
            "numero d'ecriture \"{numero}\" : il n'a pas la forme <journal>-<reference>"
        )
        .into());
    };
    if journal.trim().is_empty() || piece.trim().is_empty() {
        return Err(
            format!("numero d'ecriture \"{numero}\" : journal ou reference de piece vide").into(),
        );
    }
    Ok(piece.trim().to_string())
}

/// Une ligne d'ecriture prete a partir en base, controles faits.
struct Valeur {
    doc_ref: String,
    numero_compte: String,
    label_operation: Option<String>,
    debit: String,
    credit: String,
    montant: String,
    sens: &'static str,
    code_journal: String,
    import_key: String,
}

/// Une ecriture prete : la date, et ses lignes.
struct Prete {
    doc_date: String,
    lignes: Vec<Valeur>,
}

/// Controle et met en forme une ecriture, sans toucher a la base.
///
/// Tout ce qui peut etre refuse l'est ici, **avant** la moindre ecriture : une
/// ecriture refusee ne supprime pas non plus ce qui etait deja en base.
fn preparer(e: &Ecriture) -> Result<Prete, Box<dyn Error>> {
    if e.numero.trim().is_empty() {
        return Err("ecriture sans numero : elle n'a pas d'identite en base".into());
    }
    borner(
        e.numero.trim(),
        TAILLE_REF,
        "numero d'ecriture (colonne ref)",
    )?;
    let doc_ref = reference_piece(&e.numero)?;
    borner(&doc_ref, 300, "reference de piece (colonne doc_ref)")?;
    if !date_valide(&e.date) {
        return Err(format!(
            "ecriture {} : date \"{}\" illisible, la colonne doc_date attend AAAA-MM-JJ",
            e.numero, e.date
        )
        .into());
    }

    let mut lignes = Vec::with_capacity(e.lignes.len());
    for ligne in &e.lignes {
        lignes.push(preparer_ligne(e, ligne, &doc_ref)?);
    }
    if lignes.is_empty() {
        return Err(format!("ecriture {} : aucune ligne a ecrire", e.numero).into());
    }
    Ok(Prete {
        doc_date: e.date.clone(),
        lignes,
    })
}

fn preparer_ligne(e: &Ecriture, ligne: &Ligne, doc_ref: &str) -> Result<Valeur, Box<dyn Error>> {
    let compte = ligne.compte.trim();
    if compte.is_empty() {
        return Err(format!("ecriture {} : une ligne sans numero de compte", e.numero).into());
    }
    borner(compte, 32, "numero de compte (colonne numero_compte)")?;

    let journal = ligne.journal.trim();
    if journal.is_empty() {
        return Err(format!("ecriture {} : une ligne sans journal", e.numero).into());
    }
    borner(journal, 32, "code journal (colonne code_journal)")?;

    // `label_operation` est nullable : un libelle vide reste vide plutot que
    // d'inventer un texte.
    let libelle = if ligne.libelle.trim().is_empty() {
        e.libelle.trim().to_string()
    } else {
        ligne.libelle.trim().to_string()
    };
    borner(
        &libelle,
        255,
        "libelle d'operation (colonne label_operation)",
    )?;

    // Un seul sens : Dolibarr n'ecrit jamais un debit et un credit sur la meme
    // ligne, ni un montant negatif. `comptabilite::verifier` l'a deja exige ;
    // on repart de la ligne telle qu'elle, sans additionner.
    let (sens, montant) = if ligne.debit.is_zero() {
        ("C", ligne.credit)
    } else {
        ("D", ligne.debit)
    };

    Ok(Valeur {
        doc_ref: doc_ref.to_string(),
        numero_compte: compte.to_string(),
        label_operation: if libelle.is_empty() {
            None
        } else {
            Some(libelle)
        },
        debit: montant_sql(ligne.debit),
        credit: montant_sql(ligne.credit),
        montant: montant_sql(montant.abs()),
        sens,
        code_journal: journal.to_string(),
        import_key: cle_import_key(&e.numero, ligne),
    })
}

/// Un montant tel qu'il part en base : texte a point decimal, jamais un
/// flottant calcule par Rust (`double(24,8)` cote MySQL).
fn montant_sql(montant: Decimal) -> String {
    montant.to_string()
}

/// Options de transaction : `READ COMMITTED`.
///
/// Le niveau par defaut de MySQL (`REPEATABLE READ`) pose des **verrous
/// d'intervalle** sur `DELETE ... WHERE ref = ...` et sur le `MAX(piece_num)` :
/// deux sessions qui ecrivent deux ecritures voisines s'attendent et partent en
/// interblocage (constate en test, six entites en parallele). `READ COMMITTED`
/// ne verrouille que les lignes lues, et c'est aussi ce qu'on veut ici : les
/// lectures de `MAX(piece_num)` doivent voir ce que la transaction precedente
/// vient de valider.
fn options() -> TxOpts {
    TxOpts::default().set_isolation_level(Some(IsolationLevel::ReadCommitted))
}

const SQL_INSERER: &str = "INSERT INTO llx_accounting_bookkeeping \
     (entity, ref, piece_num, doc_date, doc_type, doc_ref, fk_doc, fk_docdet, \
      numero_compte, label_compte, label_operation, debit, credit, montant, sens, \
      code_journal, journal_label, fk_user_author, date_creation, import_key) \
     VALUES (:entity, :ref, :piece, :doc_date, :doc_type, :doc_ref, 0, 0, \
      :compte, :label_compte, :operation, :debit, :credit, :montant, :sens, \
      :journal, :journal_label, 0, NOW(), :import_key)";

const SQL_EFFACER: &str = "DELETE FROM llx_accounting_bookkeeping \
     WHERE entity = :entity AND ref = :ref";

const SQL_PIECE_SUIVANTE: &str = "SELECT COALESCE(MAX(piece_num), 0) \
     FROM llx_accounting_bookkeeping \
     WHERE entity = :entity AND code_journal = :journal";

/// Ecrit une ecriture dans `llx_accounting_bookkeeping`, et rend le nombre de
/// lignes ecrites (une par ligne d'ecriture).
///
/// L'appel est **idempotent** : les lignes deja tenues pour ce numero
/// d'ecriture sont retirees avant reinsertion (voir l'en-tete du module), donc
/// reecrire la meme ecriture laisse le meme nombre de lignes en base.
///
/// Une ecriture qui ne tient pas (`comptabilite::verifier`), une date illisible
/// ou une valeur plus longue que sa colonne sont refusees **avant** toute
/// ecriture : dans ce cas la base n'est pas touchee du tout, pas meme effacee.
pub fn ecrire(pool: &Pool, entity: i64, e: &Ecriture) -> Result<usize, Box<dyn Error>> {
    comptabilite::verifier(e)?;
    let prete = preparer(e)?;

    let mut conn = pool.get_conn()?;
    let mut tx = conn.start_transaction(options())?;
    let n = ecrire_dans(&mut tx, entity, e, &prete)?;
    tx.commit()?;
    Ok(n)
}

fn ecrire_dans(
    tx: &mut mysql::Transaction<'_>,
    entity: i64,
    e: &Ecriture,
    prete: &Prete,
) -> Result<usize, Box<dyn Error>> {
    tx.exec_drop(
        SQL_EFFACER,
        params! { "entity" => entity, "ref" => e.numero.trim() },
    )?;

    // Toutes les lignes d'une ecriture portent le meme numero de piece : c'est
    // ce qui les regroupe dans l'interface Dolibarr.
    let journal = prete
        .lignes
        .first()
        .map(|l| l.code_journal.clone())
        .unwrap_or_default();
    let max: Option<i64> = tx.exec_first(
        SQL_PIECE_SUIVANTE,
        params! { "entity" => entity, "journal" => journal },
    )?;
    let piece = max.unwrap_or(0) + 1;

    for ligne in &prete.lignes {
        tx.exec_drop(
            SQL_INSERER,
            params! {
                "entity" => entity,
                "ref" => e.numero.trim(),
                "piece" => piece,
                "doc_date" => prete.doc_date.as_str(),
                "doc_type" => DOC_TYPE,
                "doc_ref" => ligne.doc_ref.as_str(),
                "compte" => ligne.numero_compte.as_str(),
                "label_compte" => libelle_compte(&ligne.numero_compte),
                "operation" => ligne.label_operation.as_deref(),
                "debit" => ligne.debit.as_str(),
                "credit" => ligne.credit.as_str(),
                "montant" => ligne.montant.as_str(),
                "sens" => ligne.sens,
                "journal" => ligne.code_journal.as_str(),
                "journal_label" => libelle_journal(&ligne.code_journal),
                "import_key" => ligne.import_key.as_str(),
            },
        )?;
    }
    Ok(prete.lignes.len())
}

/// Ecrit tout un journal : `crate::ecritures::ecritures_du_journal` puis une
/// ecriture en base par ecriture produite.
///
/// **Tout ou rien** : les ecritures sont lues et controlees *avant* d'ouvrir la
/// transaction. Si une seule ne tient pas - desequilibre, date illisible,
/// facture dont l'en-tete ne vaut pas ses lignes - l'appel echoue et la base
/// n'a pas ete touchee. La transaction couvre ensuite l'ensemble des
/// suppressions et insertions : une panne au milieu ne laisse pas la moitie du
/// journal en base.
///
/// `limite` borne le nombre d'ecritures, pas le nombre de lignes.
pub fn ecrire_journal(
    pool: &Pool,
    entity: i64,
    journal: &str,
    limite: u32,
) -> Result<usize, Box<dyn Error>> {
    let ecritures = crate::ecritures::ecritures_du_journal(pool, entity, journal, limite)?;
    if ecritures.is_empty() {
        return Ok(0);
    }

    let mut pretes = Vec::with_capacity(ecritures.len());
    for e in &ecritures {
        comptabilite::verifier(e)?;
        pretes.push(preparer(e)?);
    }

    let mut conn = pool.get_conn()?;
    let mut tx = conn.start_transaction(options())?;
    let mut total = 0usize;
    for (e, prete) in ecritures.iter().zip(pretes.iter()) {
        total += ecrire_dans(&mut tx, entity, e, prete)?;
    }
    tx.commit()?;
    Ok(total)
}

/// Cette ecriture est-elle deja tenue en base pour cette entite ?
///
/// La question porte sur le numero d'ecriture, c'est-a-dire sur `ref`. Les
/// lignes posees par `crate::fec::generer` ne renseignent pas `ref` : elles ne
/// sont donc pas vues ici, et c'est voulu - deux ecrivains, deux marques.
pub fn deja_ecrite(pool: &Pool, entity: i64, numero: &str) -> Result<bool, Box<dyn Error>> {
    if numero.trim().is_empty() {
        return Ok(false);
    }
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_accounting_bookkeeping \
         WHERE entity = :entity AND ref = :ref",
        params! { "entity" => entity, "ref" => numero.trim() },
    )?;
    Ok(n.unwrap_or(0) > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(compte: &str, debit: &str, credit: &str) -> Ligne {
        Ligne {
            compte: compte.to_string(),
            libelle: "Facture FA1 - Tiers".to_string(),
            debit: debit.parse().unwrap_or_default(),
            credit: credit.parse().unwrap_or_default(),
            journal: "VE".to_string(),
            date: "2026-04-10".to_string(),
        }
    }

    #[test]
    fn la_cle_tient_dans_la_colonne() {
        let l = ligne("411000", "1200", "0");
        let cle = cle_import_key("VE-FA1", &l);
        assert_eq!(cle.chars().count(), TAILLE_IMPORT_KEY);
        assert!(cle.starts_with(PREFIXE_CLE));
        // Stable : deux calculs, meme valeur.
        assert_eq!(cle, cle_import_key("VE-FA1", &l));
        // Deux lignes differentes, deux cles : la TVA et le client.
        assert_ne!(cle, cle_import_key("VE-FA1", &ligne("445710", "0", "200")));
    }

    #[test]
    fn la_piece_est_tiree_du_numero() {
        assert_eq!(
            reference_piece("VE-FA2611-0015").unwrap_or_default(),
            "FA2611-0015"
        );
        assert!(reference_piece("VE").is_err());
        assert!(reference_piece("VE-").is_err());
    }

    #[test]
    fn une_date_non_iso_est_refusee() {
        assert!(date_valide("2026-04-10"));
        assert!(date_valide("2024-02-29"), "2024 est bissextile");
        assert!(!date_valide("2026-02-29"));
        assert!(!date_valide("2026-02-31"));
        assert!(!date_valide("2026-13-01"));
        assert!(!date_valide("10/04/2026"));
        assert!(!date_valide("2026-4-10"));
        assert!(!date_valide(""));
    }

    #[test]
    fn une_ligne_n_a_qu_un_sens() {
        let e = Ecriture {
            numero: "VE-FA1".to_string(),
            date: "2026-04-10".to_string(),
            libelle: "Facture FA1".to_string(),
            lignes: vec![ligne("411000", "1200", "0"), ligne("706000", "0", "1000")],
        };
        let prete = preparer(&e).unwrap();
        assert_eq!(prete.lignes[0].sens, "D");
        assert_eq!(prete.lignes[0].debit, "1200");
        assert_eq!(prete.lignes[0].credit, "0");
        assert_eq!(prete.lignes[0].montant, "1200");
        assert_eq!(prete.lignes[0].numero_compte, "411000");
        assert_eq!(libelle_compte("411000"), "Clients");
        assert_eq!(libelle_compte("999999"), "Compte");
        assert_eq!(prete.lignes[1].sens, "C");
        assert_eq!(prete.lignes[1].credit, "1000");
    }

    #[test]
    fn un_numero_trop_long_est_refuse() {
        let e = Ecriture {
            numero: format!("VE-{}", "X".repeat(TAILLE_REF)),
            date: "2026-04-10".to_string(),
            libelle: String::new(),
            lignes: vec![ligne("411000", "1", "0")],
        };
        assert!(preparer(&e).is_err(), "ref fait 30 caracteres au plus");
    }
}
