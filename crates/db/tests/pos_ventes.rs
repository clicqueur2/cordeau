//! Une vente de caisse doit exister en base : retrouvable, imprimable,
//! comptabilisable. Ces tests la relisent par les memes chemins que l'ecran de
//! caisse (facture, lignes, journal du jour) et verifient les montants au
//! centime.
//!
//! Entite dediee (900 301 / 900 302), tiers prefixes `ZZ-POS-` et un jour
//! propre a chaque test : les tests tournent en parallele sans se voir. Un garde
//! `Drop` nettoie factures, lignes, reglements et tiers du test, meme quand une
//! assertion echoue.

use std::error::Error;
use std::str::FromStr;

use mysql::prelude::Queryable;
use mysql::{params, Pool};
use pos::{Ligne, Panier};
use rust_decimal::Decimal;

/// Entite des ventes de caisse de test.
const ENTITY: i64 = 900_301;
/// Seconde entite, pour verifier le cloisonnement.
const ENTITY_AUTRE: i64 = 900_302;
/// Un jour par test : deux tests ne lisent jamais le meme journal.
const JOUR_ENCAISSEMENT: &str = "2026-03-14";
const JOUR_CUMUL: &str = "2026-03-15";
const JOUR_CREDIT: &str = "2026-03-16";
const JOUR_CLOISONNEMENT: &str = "2026-03-17";
const JOUR_REFUS: &str = "2026-03-18";

const TIERS_ENCAISSEMENT: &str = "ZZ-POS-Encaisse";
const TIERS_CUMUL: &str = "ZZ-POS-Cumul";
const TIERS_CREDIT: &str = "ZZ-POS-Credit";
const TIERS_ICI: &str = "ZZ-POS-Ici";
const TIERS_AILLEURS: &str = "ZZ-POS-Ailleurs";
const TIERS_REFUS: &str = "ZZ-POS-Refus";

fn pool_or_skip() -> Option<Pool> {
    let dsn = std::env::var("DOLIBARR_DSN").ok()?;
    match db::pool(&dsn) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("connexion impossible ({e}) : test ignore");
            None
        }
    }
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("decimal de test invalide")
}

fn tiers(pool: &Pool, entity: i64, nom: &str) -> Result<i64, Box<dyn Error>> {
    db::party::create(pool, entity, &db::party::NewParty::client(nom))
}

/// Trois articles a 19,99 EUR a 20 % : 59,97 HT, 11,99 de TVA, 71,96 TTC.
fn panier_trois_articles() -> Panier {
    let mut panier = Panier::new();
    panier
        .ajouter(
            Ligne::new(
                "ART-19",
                "Article a 19,99",
                Decimal::from(3),
                dec("19.99"),
                dec("20"),
            )
            .expect("ligne valide"),
        )
        .expect("ajout");
    panier
}

/// Un article a 10,00 EUR a 20 % : 12,00 TTC.
fn panier_dix_euros() -> Panier {
    let mut panier = Panier::new();
    panier
        .ajouter(
            Ligne::new(
                "ART-10",
                "Article a 10,00",
                Decimal::ONE,
                dec("10.00"),
                dec("20"),
            )
            .expect("ligne valide"),
        )
        .expect("ajout");
    panier
}

/// Reference externe telle qu'elle est ecrite en base.
fn ref_ext(pool: &Pool, entity: i64, facture: i64) -> Result<Option<String>, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    Ok(conn.exec_first(
        "SELECT ref_ext FROM llx_facture WHERE entity = :entity AND rowid = :rowid",
        params! { "entity" => entity, "rowid" => facture },
    )?)
}

/// Nombre de ventes de caisse rattachees a un tiers : sert a prouver qu'un
/// refus n'a rien laisse en base, sans compter les autres tests.
fn nombre_ventes_du_tiers(pool: &Pool, entity: i64, fk_soc: i64) -> Result<i64, Box<dyn Error>> {
    let mut conn = pool.get_conn()?;
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM llx_facture \
         WHERE entity = :entity AND fk_soc = :soc AND ref_ext LIKE 'POS-%'",
        params! { "entity" => entity, "soc" => fk_soc },
    )?;
    Ok(n.unwrap_or(0))
}

/// Nettoyage garanti a la sortie, y compris sur panique d'assertion.
///
/// Ne supprime que les factures et le tiers **de ce test** : le nom du tiers
/// tient lieu de perimetre, comme dans les autres tests de facturation.
struct Garde {
    pool: Pool,
    cibles: Vec<(i64, String)>,
}

impl Garde {
    fn new(pool: &Pool, cibles: &[(i64, &str)]) -> Result<Self, Box<dyn Error>> {
        let garde = Self {
            pool: pool.clone(),
            cibles: cibles.iter().map(|(e, n)| (*e, (*n).to_string())).collect(),
        };
        garde.nettoyer()?;
        Ok(garde)
    }

    fn nettoyer(&self) -> Result<(), Box<dyn Error>> {
        for (entity, nom) in &self.cibles {
            for f in db::invoice::list(&self.pool, *entity, 100_000, 0)? {
                let Some(fk_soc) = f.fk_soc else {
                    continue;
                };
                let appartient = db::party::get(&self.pool, *entity, fk_soc)?
                    .map(|p| p.nom == *nom)
                    .unwrap_or(false);
                if appartient {
                    db::invoice::delete(&self.pool, *entity, f.rowid)?;
                }
            }
            for p in db::party::list(&self.pool, *entity, 10_000, 0)? {
                if p.nom == *nom {
                    db::party::delete(&self.pool, *entity, p.rowid)?;
                }
            }
        }
        Ok(())
    }
}

impl Drop for Garde {
    fn drop(&mut self) {
        if let Err(e) = self.nettoyer() {
            eprintln!("nettoyage impossible : {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// 1. La vente encaissee existe, au centime, et se retrouve au journal du jour.
// ---------------------------------------------------------------------------

#[test]
fn une_vente_encaissee_devient_une_facture_validee_et_payee() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = Garde::new(&pool, &[(ENTITY, TIERS_ENCAISSEMENT)])?;
    let fk_soc = tiers(&pool, ENTITY, TIERS_ENCAISSEMENT)?;

    let panier = panier_trois_articles();
    assert_eq!(panier.total_ht(), dec("59.97"));
    assert_eq!(panier.total_tva(), dec("11.99"));
    assert_eq!(panier.total_ttc(), dec("71.96"));

    let facture = db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        Some(fk_soc),
        &panier,
        "especes",
        dec("71.96"),
        JOUR_ENCAISSEMENT,
    )?;

    let f = db::invoice::get(&pool, ENTITY, facture)?.expect("facture de caisse relue");
    assert_eq!(f.entity, ENTITY);
    assert_eq!(f.fk_soc, Some(fk_soc));
    assert_eq!(
        f.type_,
        db::invoice::TYPE_FACTURE,
        "une vente est une facture"
    );
    assert_eq!(f.total_ht, Some(dec("59.97")));
    assert_eq!(f.total_tva, Some(dec("11.99")));
    assert_eq!(
        f.total_ttc,
        Some(dec("71.96")),
        "3 x 19,99 a 20 % font 71,96 TTC"
    );

    // Une ligne de facture par ligne de panier, article compris, unite posee.
    let lignes = db::invoice::lines(&pool, ENTITY, facture)?;
    assert_eq!(lignes.len(), 1, "une ligne de panier, une ligne de facture");
    assert_eq!(lignes[0].label.as_deref(), Some("Article a 19,99"));
    assert_eq!(lignes[0].description.as_deref(), Some("ART-19"));
    assert_eq!(lignes[0].qty, Some(Decimal::from(3)));
    assert_eq!(lignes[0].subprice, Some(dec("19.99")));
    assert_eq!(lignes[0].tva_tx, Some(dec("20")));
    assert_eq!(lignes[0].total_ttc, Some(dec("71.96")));
    assert!(
        lignes[0].fk_unit.is_some(),
        "l'unite (piece) doit etre posee sur la ligne"
    );

    // La garde : une reference externe par vente, datee du jour de la vente.
    let reference = ref_ext(&pool, ENTITY, facture)?.expect("reference externe posee");
    assert!(
        reference.starts_with("POS-"),
        "reference externe inattendue : {reference}"
    );
    assert!(
        reference.contains("20260314"),
        "la reference doit porter la date de la vente : {reference}"
    );

    // L'argent est entre : la vente est soldee (Dolibarr refuse de regler un
    // brouillon, une vente de caisse encaissee est un document emis).
    assert_eq!(f.fk_statut, db::invoice::STATUT_PAYEE);
    assert_eq!(f.paye, 1, "la facture doit etre marquee payee");
    let solde = db::payment::balance(&pool, ENTITY, facture)?;
    assert_eq!(solde.total_ttc, dec("71.96"));
    assert_eq!(solde.regle, dec("71.96"), "le reglement porte le TTC");
    assert_eq!(solde.reste, Decimal::ZERO);
    assert!(solde.soldee);

    // Le journal du jour la retrouve, avec son moyen de paiement.
    let ventes = db::pos_ventes::ventes_du_jour(&pool, ENTITY, JOUR_ENCAISSEMENT)?;
    assert_eq!(ventes.len(), 1);
    assert_eq!(ventes[0].facture_id, facture);
    assert_eq!(ventes[0].reference, f.ref_);
    assert_eq!(ventes[0].total_ttc, dec("71.96"));
    assert_eq!(ventes[0].paye, dec("71.96"));
    assert!(
        !ventes[0].moyen.is_empty(),
        "le moyen de paiement doit etre relu en base"
    );

    // La caisse peut dire « j'ai encaisse 71,96 en 1 vente » sans relire les lignes.
    assert_eq!(
        db::pos_ventes::total_du_jour(&pool, ENTITY, JOUR_ENCAISSEMENT)?,
        (dec("71.96"), 1)
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Deux ventes du meme jour s'additionnent, sans collision de reference.
// ---------------------------------------------------------------------------

#[test]
fn deux_ventes_du_meme_jour_se_cumulent() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = Garde::new(&pool, &[(ENTITY, TIERS_CUMUL)])?;
    let fk_soc = tiers(&pool, ENTITY, TIERS_CUMUL)?;

    let premiere = db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        Some(fk_soc),
        &panier_trois_articles(),
        "especes",
        dec("71.96"),
        JOUR_CUMUL,
    )?;
    let seconde = db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        Some(fk_soc),
        &panier_dix_euros(),
        "cb",
        // 20,00 tendus pour 12,00 dus : le rendu de 8,00 ne rentre pas en caisse.
        dec("20.00"),
        JOUR_CUMUL,
    )?;

    let ref_premiere = ref_ext(&pool, ENTITY, premiere)?.expect("reference 1");
    let ref_seconde = ref_ext(&pool, ENTITY, seconde)?.expect("reference 2");
    assert_ne!(
        ref_premiere, ref_seconde,
        "deux ventes ne peuvent pas porter la meme reference externe"
    );

    let solde = db::payment::balance(&pool, ENTITY, seconde)?;
    assert_eq!(
        solde.regle,
        dec("12.00"),
        "le reglement porte le TTC, jamais la coupure tendue"
    );
    assert!(solde.soldee);

    let ventes = db::pos_ventes::ventes_du_jour(&pool, ENTITY, JOUR_CUMUL)?;
    assert_eq!(ventes.len(), 2);
    assert_eq!(ventes[0].total_ttc, dec("71.96"));
    assert_eq!(ventes[1].total_ttc, dec("12.00"));
    assert_eq!(ventes[1].paye, dec("12.00"));
    assert_eq!(ventes[1].moyen.to_ascii_uppercase(), "CB");

    // 71,96 + 12,00 = 83,96 en 2 ventes, au centime.
    assert_eq!(
        db::pos_ventes::total_du_jour(&pool, ENTITY, JOUR_CUMUL)?,
        (dec("83.96"), 2)
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Une vente non encaissee existe quand meme, mais reste en brouillon.
// ---------------------------------------------------------------------------

#[test]
fn une_vente_non_encaissee_reste_en_brouillon_sans_reglement() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = Garde::new(&pool, &[(ENTITY, TIERS_CREDIT)])?;
    let fk_soc = tiers(&pool, ENTITY, TIERS_CREDIT)?;

    // 50,00 tendus pour 71,96 dus : la vente n'est pas soldee.
    let facture = db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        Some(fk_soc),
        &panier_trois_articles(),
        "especes",
        dec("50.00"),
        JOUR_CREDIT,
    )?;

    let f = db::invoice::get(&pool, ENTITY, facture)?.expect("facture relue");
    assert_eq!(f.fk_statut, db::invoice::STATUT_BROUILLON);
    assert_eq!(f.paye, 0);
    assert!(
        db::payment::list_for_invoice(&pool, ENTITY, facture)?.is_empty(),
        "aucun reglement ne doit etre ecrit pour une vente non soldee"
    );

    // La vente reste visible au journal, avec 0,00 encaisse.
    let ventes = db::pos_ventes::ventes_du_jour(&pool, ENTITY, JOUR_CREDIT)?;
    assert_eq!(ventes.len(), 1);
    assert_eq!(ventes[0].total_ttc, dec("71.96"));
    assert_eq!(ventes[0].paye, Decimal::ZERO);
    assert_eq!(ventes[0].moyen, "");
    assert_eq!(
        db::pos_ventes::total_du_jour(&pool, ENTITY, JOUR_CREDIT)?,
        (dec("71.96"), 1)
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Cloisonnement : une vente n'existe que dans son entite.
// ---------------------------------------------------------------------------

#[test]
fn le_cloisonnement_par_entite_isole_les_ventes() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = Garde::new(
        &pool,
        &[(ENTITY, TIERS_ICI), (ENTITY_AUTRE, TIERS_AILLEURS)],
    )?;
    let ici = tiers(&pool, ENTITY, TIERS_ICI)?;
    let ailleurs = tiers(&pool, ENTITY_AUTRE, TIERS_AILLEURS)?;

    let facture_ici = db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        Some(ici),
        &panier_trois_articles(),
        "especes",
        dec("71.96"),
        JOUR_CLOISONNEMENT,
    )?;
    let facture_ailleurs = db::pos_ventes::enregistrer(
        &pool,
        ENTITY_AUTRE,
        Some(ailleurs),
        &panier_dix_euros(),
        "especes",
        dec("12.00"),
        JOUR_CLOISONNEMENT,
    )?;
    assert_ne!(facture_ici, facture_ailleurs);

    let ventes_ici = db::pos_ventes::ventes_du_jour(&pool, ENTITY, JOUR_CLOISONNEMENT)?;
    assert_eq!(ventes_ici.len(), 1, "une seule vente dans cette entite");
    assert_eq!(ventes_ici[0].facture_id, facture_ici);
    assert_eq!(
        db::pos_ventes::total_du_jour(&pool, ENTITY, JOUR_CLOISONNEMENT)?,
        (dec("71.96"), 1)
    );

    let ventes_ailleurs = db::pos_ventes::ventes_du_jour(&pool, ENTITY_AUTRE, JOUR_CLOISONNEMENT)?;
    assert_eq!(ventes_ailleurs.len(), 1);
    assert_eq!(ventes_ailleurs[0].facture_id, facture_ailleurs);
    assert_eq!(
        db::pos_ventes::total_du_jour(&pool, ENTITY_AUTRE, JOUR_CLOISONNEMENT)?,
        (dec("12.00"), 1)
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Ce qui ne peut pas etre une vente est refuse, sans rien laisser en base.
// ---------------------------------------------------------------------------

#[test]
fn refuse_les_ventes_invalides() -> Result<(), Box<dyn Error>> {
    let Some(pool) = pool_or_skip() else {
        return Ok(());
    };
    let _garde = Garde::new(&pool, &[(ENTITY, TIERS_REFUS)])?;
    let fk_soc = tiers(&pool, ENTITY, TIERS_REFUS)?;

    // Panier vide.
    assert!(db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        Some(fk_soc),
        &Panier::new(),
        "especes",
        dec("10.00"),
        JOUR_REFUS
    )
    .is_err());

    // Pas de tiers : Dolibarr n'a pas de facture sans tiers, on n'en invente pas.
    assert!(db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        None,
        &panier_dix_euros(),
        "especes",
        dec("12.00"),
        JOUR_REFUS
    )
    .is_err());

    // Date qui n'est pas AAAA-MM-JJ.
    assert!(db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        Some(fk_soc),
        &panier_dix_euros(),
        "especes",
        dec("12.00"),
        "14/03/2026"
    )
    .is_err());

    // Moyen de paiement inconnu du dictionnaire.
    assert!(db::pos_ventes::enregistrer(
        &pool,
        ENTITY,
        Some(fk_soc),
        &panier_dix_euros(),
        "bitcoin",
        dec("12.00"),
        JOUR_REFUS
    )
    .is_err());

    // Aucun de ces refus n'a ecrit de vente pour ce tiers.
    assert_eq!(nombre_ventes_du_tiers(&pool, ENTITY, fk_soc)?, 0);

    Ok(())
}
