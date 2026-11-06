//! Cas d'usage partages par le CLI et le web.
//!
//! Cette couche ne fait ni SQL ni mise en page : elle prend les objets de `db`,
//! les traduit en vues `pdf` et en contexte `legal`, et rend le document.

use std::collections::BTreeMap;
use std::error::Error;

use db::Pool;
use rust_decimal::Decimal;

/// Le nom du produit, **defini une seule fois**.
///
/// Tout ce qui l'affiche — le titre des pages, la banniere du binaire — lit
/// cette constante. Changer le nom du logiciel, c'est changer cette ligne ; les
/// documents se renomment avec `tools/renommer.ps1`.
///
/// A ne pas confondre avec la **societe** : le produit s'appelle Cordeau, la
/// societe qui l'edite s'appelle Pierre Angulaire, et c'est la base qui le dit
/// (`llx_societe`). Deux choses, deux sources.
pub const PRODUIT: &str = "Cordeau";

/// La mention affichee sous le nom : ce que le produit est, en trois mots.
pub const SOUS_TITRE: &str = "gestion et facturation";

/// Les marges du PDF, lues dans les constantes Dolibarr.
///
/// `MAIN_PDF_MARGIN_LEFT`, `_RIGHT`, `_TOP`, `_BOTTOM` sont les noms du coeur.
/// Le gabarit n'a qu'une largeur : on prend **la plus grande** des deux marges
/// laterales. Une marge trop petite se voit — le tableau touche le bord — alors
/// qu'une marge trop grande ne fait que reduire la largeur utile.
///
/// Chaque valeur est bornee : une constante negative ou absurde (un `0` oublie,
/// un `-5` recopie) donnerait un PDF illisible. On retombe alors sur le defaut
/// du coeur, et rien ne casse.
pub fn marges_pdf(pool: &db::Pool) -> pdf::Marges {
    let lire = |cle: &str| -> Option<f32> {
        db::company::get_const(pool, cle)
            .ok()
            .flatten()
            .and_then(|v| v.trim().replace(',', ".").parse::<f32>().ok())
    };
    let defaut = pdf::Marges::default();
    let borne = |v: Option<f32>, defaut: f32| -> f32 {
        match v {
            Some(x) if (2.0..=60.0).contains(&x) => x,
            _ => defaut,
        }
    };

    let gauche = borne(lire("MAIN_PDF_MARGIN_LEFT"), defaut.x);
    let droite = borne(lire("MAIN_PDF_MARGIN_RIGHT"), defaut.x);
    pdf::Marges {
        x: gauche.max(droite),
        top: borne(lire("MAIN_PDF_MARGIN_TOP"), defaut.top),
        bottom: borne(lire("MAIN_PDF_MARGIN_BOTTOM"), defaut.bottom),
    }
}

/// La configuration SMTP, lue dans `llx_const`.
///
/// Elle vit ici et non dans la couche web : le courriel part aussi bien d'un
/// ecran que d'une tache planifiee, et deux copies d'un reglage finissent
/// toujours par diverger. La variable `DOLIBARR_SMTP_PASSWORD` l'emporte sur la
/// base : un secret n'a pas a trainer dans une table.
pub fn smtp_config(pool: &db::Pool) -> Result<mail::Config, String> {
    let const_ou = |cle: &str| {
        db::company::get_const(pool, cle)
            .ok()
            .flatten()
            .unwrap_or_default()
    };

    let hote = const_ou("DOLIBARR_RS_SMTP_HOST");
    if hote.trim().is_empty() {
        return Err(
            "serveur SMTP non renseigné (Paramètres > Constantes : DOLIBARR_RS_SMTP_HOST)".into(),
        );
    }
    let port = const_ou("DOLIBARR_RS_SMTP_PORT")
        .parse::<u16>()
        .unwrap_or(587);

    let mut mot_de_passe = const_ou("DOLIBARR_RS_SMTP_MOTDEPASSE");
    if let Ok(v) = std::env::var("DOLIBARR_SMTP_PASSWORD") {
        if !v.is_empty() {
            mot_de_passe = v;
        }
    }

    let config = mail::Config {
        hote: hote.trim().to_string(),
        port,
        utilisateur: const_ou("DOLIBARR_RS_SMTP_UTILISATEUR").trim().to_string(),
        mot_de_passe,
        expediteur: const_ou("DOLIBARR_RS_EMAIL_EXPEDITEUR").trim().to_string(),
        nom_expediteur: const_ou("MAIN_INFO_SOCIETE_NOM").trim().to_string(),
        securite: match const_ou("DOLIBARR_RS_SMTP_SECURITE").trim() {
            "aucune" => mail::Securite::Aucune,
            "directe" => mail::Securite::Directe,
            "starttls" => mail::Securite::Starttls,
            _ => mail::Config::securite_pour(port),
        },
        delai: 20,
    };

    if config.expediteur.is_empty() {
        return Err("adresse d'expédition non renseignée (Paramètres > Constantes : DOLIBARR_RS_EMAIL_EXPEDITEUR)".into());
    }
    Ok(config)
}

/// Le modele de PDF d'un type de piece.
///
/// La couche donnees dit **quel code** est enregistre ; c'est ici qu'on le
/// traduit en modele de rendu. Un code inconnu retombe sur le standard plutot
/// que d'echouer : un document doit pouvoir sortir meme si un reglage a ete
/// saisi de travers.
fn modele_pdf(
    pool: &db::Pool,
    entity: i64,
    objet: db::pdf_modele::Objet,
) -> Result<pdf::Modele, Box<dyn Error>> {
    Ok(db::pdf_modele::modele(pool, entity, objet)?
        .and_then(|c| pdf::Modele::depuis_code(&c))
        .unwrap_or(pdf::Modele::Standard))
}

pub fn fmt_dec(d: Option<Decimal>) -> String {
    d.map(|x| x.normalize().to_string()).unwrap_or_default()
}

/// Montant en euros, ecriture francaise : `3 736,00 €`.
///
/// Le symbole et les deux decimales sont obligatoires sur une facture ; le
/// separateur de milliers est une espace insecable, pour qu'un montant ne soit
/// jamais coupe par un retour a la ligne.
pub fn fmt_eur(d: Option<Decimal>) -> String {
    let Some(v) = d else {
        return String::new();
    };
    let arrondi = money::round_money(v);
    let negatif = arrondi.is_sign_negative() && arrondi != Decimal::ZERO;
    let texte = format!("{:.2}", arrondi.abs());
    let (entier, decimales) = texte.split_once('.').unwrap_or((texte.as_str(), "00"));
    let mut groupes = String::new();
    for (i, c) in entier.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            groupes.push('\u{a0}');
        }
        groupes.push(c);
    }
    let entier: String = groupes.chars().rev().collect();
    format!(
        "{}{entier},{decimales}\u{a0}€",
        if negatif { "-" } else { "" }
    )
}

/// Quantite en ecriture francaise : `2,5` (virgule, zeros inutiles retires).
pub fn fmt_qte(d: Option<Decimal>) -> String {
    fmt_dec(d).replace('.', ",")
}

/// Date au format francais : `2026-11-02` devient `02/11/2026`.
///
/// Le gabarit n'a pas a connaitre le format ISO : une facture francaise porte
/// des dates en jj/mm/aaaa, comme le faisait le document d'origine.
pub fn fmt_date_fr(iso: Option<&str>) -> String {
    let Some(t) = iso.map(str::trim).filter(|t| t.len() >= 10) else {
        return String::new();
    };
    let (annee, mois, jour) = (&t[0..4], &t[5..7], &t[8..10]);
    if annee.chars().all(|c| c.is_ascii_digit())
        && mois.chars().all(|c| c.is_ascii_digit())
        && jour.chars().all(|c| c.is_ascii_digit())
    {
        format!("{jour}/{mois}/{annee}")
    } else {
        t.to_string()
    }
}

pub fn societe_legale(c: &db::company::Company) -> legal::Societe {
    legal::Societe {
        nom: c.nom.clone(),
        // Le libelle resolu prime : la constante Dolibarr porte un code.
        forme_juridique: c
            .forme_juridique_libelle
            .clone()
            .or_else(|| c.forme_juridique.clone()),
        adresse: c.adresse.clone(),
        code_postal: c.code_postal.clone(),
        ville: c.ville.clone(),
        pays: c.code_pays(),
        siret: c.siret.clone(),
        siren: c.siren.clone(),
        tva_intra: c.tva_intra.clone(),
        capital: c.capital.as_deref().map(capital_lisible),
        rcs: c.rcs.clone(),
        email: c.email.clone(),
        telephone: c.telephone.clone(),
        regime_tva: if c.franchise_en_base {
            legal::RegimeTva::FranchiseEnBase
        } else {
            legal::RegimeTva::Assujetti
        },
        assurance: c.assurance_assureur.as_ref().map(|a| legal::Assurance {
            assureur: a.clone(),
            police: c.assurance_police.clone().unwrap_or_default(),
            couverture: c.assurance_couverture.clone().unwrap_or_default(),
        }),
        assurance_texte: c.assurance_mention.clone(),
    }
}

/// Le capital tel qu'il doit s'ecrire : `5100` devient `5 100 EUR`.
///
/// Dolibarr stocke un nombre nu, sans devise. Un capital sans unite sur une
/// facture se lit mal, et « 5100 » colle a la suite du nom ne se lit pas du
/// tout. Une valeur qui n'est pas un nombre est rendue telle quelle : on ne
/// reformate pas ce qu'on ne comprend pas.
fn capital_lisible(brut: &str) -> String {
    let t = brut.trim();
    let nombre: String = t.replace([' ', '\u{a0}'], "").replace(',', ".");
    let Ok(d) = nombre.parse::<rust_decimal::Decimal>() else {
        return t.to_string();
    };
    let entier = d.trunc().to_string();
    let mut out = String::new();
    for (i, c) in entier.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    let groupe: String = out.chars().rev().collect();
    if d.fract() == rust_decimal::Decimal::ZERO {
        format!("{groupe}\u{a0}€")
    } else {
        format!(
            "{groupe},{}\u{a0}€",
            d.fract().to_string().trim_start_matches("0.")
        )
    }
}

/// Professionnel des qu'il porte un SIRET ou un numero de TVA intracommunautaire.
///
/// La chaine vide ne compte pas : Dolibarr ecrit `''` dans ces colonnes pour
/// dire « rien ». La lire comme une valeur rendait professionnel un particulier
/// sur deux, et faisait refuser ses factures.
pub fn client_legal(p: &db::party::Party) -> legal::Client {
    let renseigne =
        |v: &Option<String>| v.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    legal::Client {
        nom: p.nom.clone(),
        siret: p.siret.clone(),
        tva_intra: p.tva_intra.clone(),
        est_professionnel: renseigne(&p.siret) || renseigne(&p.tva_intra),
        pays: p.fk_pays.map(|id| {
            if id == 1 {
                "FR".to_string()
            } else {
                id.to_string()
            }
        }),
    }
}

pub fn conditions_legales(c: &db::company::Company, origine: Option<&str>) -> legal::Conditions {
    legal::Conditions {
        delai_jours: c.delai_paiement_jours,
        tva_sur_encaissements: c.tva_sur_encaissements,
        tva_taux_reduit: c.tva_taux_reduit,
        origine: origine.map(|s| s.to_string()),
        ..Default::default()
    }
}

/// Un libelle qui commence par « Règlement » ferait doublon avec
/// l'intitule de la ligne du PDF : on retire le mot de tete.
pub fn nettoyer_condition(libelle: &str) -> String {
    let bas = libelle.to_lowercase();
    if bas.starts_with("règlement ") {
        libelle["règlement ".len()..].trim().to_string()
    } else if bas.starts_with("reglement ") {
        libelle["reglement ".len()..].trim().to_string()
    } else {
        libelle.to_string()
    }
}

/// Titre imprimé sur le PDF : le type Dolibarr ne dit pas tout.
pub fn titre_document(type_: i16, situation: Option<i32>) -> Option<String> {
    if type_ != db::invoice::TYPE_SITUATION {
        return None;
    }
    match situation {
        Some(n) => Some(format!("Facture de situation n° {n}")),
        None => Some("Facture de situation".to_string()),
    }
}

pub fn vendeur_pdf(c: &db::company::Company) -> pdf::PartyView {
    let legale = societe_legale(c);
    pdf::PartyView {
        name: c.nom.clone(),
        address: c.adresse.clone(),
        zip: c.code_postal.clone(),
        town: c.ville.clone(),
        siret: c.siret.clone(),
        tva_intra: c.tva_intra.clone(),
        legal_line: legal::entete_legal(&legale),
    }
}

pub fn coordonnees_bancaires(c: &db::company::Company) -> Option<String> {
    let iban = c.iban.as_deref().map(str::trim).filter(|s| !s.is_empty())?;
    let bic = c.bic.as_deref().map(str::trim).unwrap_or("");
    Some(if bic.is_empty() {
        format!("RIB — IBAN {iban}")
    } else {
        format!("RIB — IBAN {iban} — BIC {bic}")
    })
}

/// RIB du compte designe par le document (`llx_facture.fk_account`), sinon
/// celui de la societe. Dolibarr imprime le compte de la facture.
pub fn rib_du_document(
    pool: &Pool,
    entity: i64,
    fk_account: Option<i64>,
    emit: &db::company::Company,
) -> Option<String> {
    if let Some(id) = fk_account {
        if let Ok(Some(ligne)) = db::banque::compte_libelle(pool, entity, id) {
            return Some(ligne);
        }
    }
    coordonnees_bancaires(emit)
}

pub fn client_pdf(p: &db::party::Party) -> pdf::PartyView {
    pdf::PartyView {
        name: p.nom.clone(),
        address: p.address.clone(),
        zip: p.zip.clone(),
        town: p.town.clone(),
        siret: p.siret.clone(),
        tva_intra: p.tva_intra.clone(),
        legal_line: None,
    }
}

/// Ligne brute, quelle que soit son origine (devis ou facture).
struct LigneBrute {
    est_section: bool,
    label: Option<String>,
    description: Option<String>,
    qty: Option<Decimal>,
    subprice: Option<Decimal>,
    remise: Option<Decimal>,
    tva: Option<Decimal>,
    total: Option<Decimal>,
    fk_unit: Option<i64>,
}

/// Sections en bande pleine largeur, sous-total calcule a la fin de chaque section.
fn composer_lignes(
    lignes: impl Iterator<Item = LigneBrute>,
    unites: &std::collections::BTreeMap<i64, String>,
) -> Vec<pdf::LineView> {
    let mut out = Vec::new();
    let mut sous_total = Decimal::ZERO;
    let mut en_section = false;

    for l in lignes {
        if l.est_section {
            if sous_total != Decimal::ZERO {
                out.push(pdf::LineView {
                    kind: pdf::LineKind::Subtotal,
                    label: "Sous-total".to_string(),
                    total_ht: fmt_eur(Some(sous_total)),
                    ..Default::default()
                });
                sous_total = Decimal::ZERO;
            }
            en_section = true;
            out.push(pdf::LineView {
                kind: pdf::LineKind::Section,
                label: l.label.unwrap_or_default(),
                ..Default::default()
            });
            continue;
        }
        sous_total += l.total.unwrap_or_default();
        out.push(pdf::LineView {
            kind: pdf::LineKind::Normal,
            label: l.label.unwrap_or_default(),
            description: l.description,
            qty: fmt_qte(l.qty),
            unit: l
                .fk_unit
                .and_then(|id| unites.get(&id).cloned())
                .unwrap_or_default(),
            unit_price_ht: fmt_eur(l.subprice),
            discount_percent: format!("{} %", fmt_qte(l.remise)),
            vat_rate: format!("{} %", fmt_qte(l.tva)),
            total_ht: fmt_eur(l.total),
        });
    }
    if en_section && sous_total != Decimal::ZERO {
        out.push(pdf::LineView {
            kind: pdf::LineKind::Subtotal,
            label: "Sous-total".to_string(),
            total_ht: fmt_eur(Some(sous_total)),
            ..Default::default()
        });
    }
    out
}

pub fn lignes_devis(
    lignes: &[db::proposal::ProposalLine],
    unites: &std::collections::BTreeMap<i64, String>,
) -> Vec<pdf::LineView> {
    composer_lignes(
        lignes.iter().map(|l| LigneBrute {
            est_section: l.is_section(),
            label: l.label.clone(),
            description: l.description.clone(),
            qty: l.qty,
            subprice: l.subprice,
            remise: l.remise_percent,
            tva: l.tva_tx,
            total: l.total_ht,
            fk_unit: l.fk_unit,
        }),
        unites,
    )
}

pub fn lignes_facture(
    lignes: &[db::invoice::InvoiceLine],
    unites: &std::collections::BTreeMap<i64, String>,
) -> Vec<pdf::LineView> {
    composer_lignes(
        lignes.iter().map(|l| LigneBrute {
            est_section: l.is_section(),
            label: l.label.clone(),
            description: l.description.clone(),
            qty: l.qty,
            subprice: l.subprice,
            remise: l.remise_percent,
            tva: l.tva_tx,
            total: l.total_ht,
            fk_unit: l.fk_unit,
        }),
        unites,
    )
}
pub struct RenduPdf {
    pub bytes: Vec<u8>,
    pub pages: usize,
}

pub fn devis_pdf(pool: &Pool, entity: i64, id: i64) -> Result<RenduPdf, Box<dyn Error>> {
    let p = db::proposal::get(pool, entity, id)?.ok_or("devis introuvable")?;
    let soc = db::party::get(pool, entity, p.fk_soc.ok_or("devis sans tiers")?)?
        .ok_or("tiers introuvable")?;
    let lignes = db::proposal::lines(pool, entity, id)?;
    let emit = db::company::get(pool, entity)?;

    let mut vat: BTreeMap<String, Decimal> = BTreeMap::new();
    for l in &lignes {
        *vat.entry(fmt_qte(l.tva_tx)).or_default() += l.total_tva.unwrap_or_default();
    }

    let doc = pdf::DocumentView {
        kind: pdf::DocKind::Quote,
        title_override: None,
        reference: p.ref_.clone(),
        date: fmt_date_fr(p.datep.as_deref()),
        due_date: None,
        valid_until: Some(fmt_date_fr(p.fin_validite.as_deref())).filter(|d| !d.is_empty()),
        // Elements du contrat : condition de reglement et acompte demande.
        payment_terms: p
            .fk_cond_reglement
            .and_then(|id| db::reglement::condition(pool, entity, id).ok().flatten())
            .map(|c| nettoyer_condition(&c.libelle_fr())),
        deposit: p.deposit_percent.map(|pct| {
            format!(
                "{} % — {}",
                fmt_qte(Some(pct)),
                fmt_eur(Some(money::round_money(
                    p.total_ttc.unwrap_or_default() * pct / Decimal::from(100)
                )))
            )
        }),
        customer_ref: None,
        service_date: None,
        bank: coordonnees_bancaires(&emit),
        cgv: emit.cgv.clone(),
        seller: vendeur_pdf(&emit),
        customer: client_pdf(&soc),
        lines: lignes_devis(&lignes, &db::dict::unites_libelle(pool).unwrap_or_default()),
        total_ht: fmt_eur(p.total_ht),
        total_vat: fmt_eur(p.total_tva),
        total_ttc: fmt_eur(p.total_ttc),
        vat_breakdown: vat
            .into_iter()
            .map(|(t, m)| (format!("{t} %"), fmt_eur(Some(m))))
            .collect(),
        mentions: legal::mentions(
            legal::DocType::Devis,
            &societe_legale(&emit),
            &client_legal(&soc),
            &conditions_legales(&emit, None),
        ),
        // Le document part chez le client : seules les notes publiques sortent.
        // Une note interne sur une facture, c'est une fuite.
        notes: p.note_public.clone().filter(|n| !n.trim().is_empty()),
        margins: marges_pdf(pool),
    };

    let modele = modele_pdf(pool, entity, db::pdf_modele::Objet::Devis)?;
    let r = pdf::render_avec_modele(&doc, modele)?;
    Ok(RenduPdf {
        bytes: r.bytes,
        pages: r.pages,
    })
}

/// Rend la facture et la depose dans la GED (`llx_ecm_files`).
pub fn archiver_facture(pool: &Pool, entity: i64, id: i64) -> Result<i64, Box<dyn Error>> {
    let rendu = facture_pdf(pool, entity, id)?;
    let f = db::invoice::get(pool, entity, id)?.ok_or("facture introuvable")?;
    // Dolibarr range factures et avoirs sous le meme module `facture`.
    let modulepart = "facture";
    db::ged::supprimer_pour_element(pool, entity, "facture", id)?;
    db::ged::archiver(
        pool,
        entity,
        &db::ged::DocumentAAchiver {
            modulepart,
            reference: &f.ref_,
            object_type: "facture",
            object_id: id,
            label: &format!("Facture {}", f.ref_),
            contenu: &rendu.bytes,
            extension: "pdf",
        },
    )
}

/// Rend le devis et le depose dans la GED.
pub fn archiver_devis(pool: &Pool, entity: i64, id: i64) -> Result<i64, Box<dyn Error>> {
    let rendu = devis_pdf(pool, entity, id)?;
    let d = db::proposal::get(pool, entity, id)?.ok_or("devis introuvable")?;
    db::ged::supprimer_pour_element(pool, entity, "propal", id)?;
    db::ged::archiver(
        pool,
        entity,
        &db::ged::DocumentAAchiver {
            modulepart: "propale",
            reference: &d.ref_,
            object_type: "propal",
            object_id: id,
            label: &format!("Devis {}", d.ref_),
            contenu: &rendu.bytes,
            extension: "pdf",
        },
    )
}

/// Rend le bon de commande client.
pub fn commande_pdf(pool: &Pool, entity: i64, id: i64) -> Result<RenduPdf, Box<dyn Error>> {
    let c = db::commande::get(pool, entity, id)?.ok_or("commande introuvable")?;
    let soc = db::party::get(pool, entity, c.fk_soc)?.ok_or("tiers introuvable")?;
    let lignes = db::commande::lines(pool, entity, id)?;
    let emit = db::company::get(pool, entity)?;
    let unites = db::dict::unites_libelle(pool).unwrap_or_default();

    let mut vat: BTreeMap<String, Decimal> = BTreeMap::new();
    for l in &lignes {
        *vat.entry(fmt_qte(l.tva_tx)).or_default() += l.total_tva.unwrap_or_default();
    }
    let condition = c
        .fk_cond_reglement
        .and_then(|cid| db::reglement::condition(pool, entity, cid).ok().flatten());

    let doc = pdf::DocumentView {
        kind: pdf::DocKind::Order,
        title_override: None,
        reference: c.ref_.clone(),
        date: fmt_date_fr(c.date_commande.as_deref()),
        due_date: None,
        valid_until: None,
        payment_terms: condition.map(|x| nettoyer_condition(&x.libelle_fr())),
        deposit: c.deposit_percent.map(|pct| {
            format!(
                "{} % — {}",
                fmt_qte(Some(pct)),
                fmt_eur(Some(money::round_money(
                    c.total_ttc.unwrap_or_default() * pct / Decimal::from(100)
                )))
            )
        }),
        customer_ref: None,
        service_date: None,
        bank: coordonnees_bancaires(&emit),
        cgv: emit.cgv.clone(),
        seller: vendeur_pdf(&emit),
        customer: client_pdf(&soc),
        lines: composer_lignes(
            lignes.iter().map(|l| LigneBrute {
                est_section: l.is_section(),
                label: l.label.clone(),
                description: l.description.clone(),
                qty: l.qty,
                subprice: l.subprice,
                remise: l.remise_percent,
                tva: l.tva_tx,
                total: l.total_ht,
                fk_unit: l.fk_unit,
            }),
            &unites,
        ),
        total_ht: fmt_eur(c.total_ht),
        total_vat: fmt_eur(c.total_tva),
        total_ttc: fmt_eur(c.total_ttc),
        vat_breakdown: vat
            .into_iter()
            .map(|(t, m)| (format!("{t} %"), fmt_eur(Some(m))))
            .collect(),
        mentions: legal::mentions(
            legal::DocType::Commande,
            &societe_legale(&emit),
            &client_legal(&soc),
            &conditions_legales(&emit, None),
        ),
        notes: c.note_public.clone().filter(|n| !n.trim().is_empty()),
        margins: marges_pdf(pool),
    };

    let modele = modele_pdf(pool, entity, db::pdf_modele::Objet::Commande)?;
    let r = pdf::render_avec_modele(&doc, modele)?;
    Ok(RenduPdf {
        bytes: r.bytes,
        pages: r.pages,
    })
}

pub fn facture_pdf(pool: &Pool, entity: i64, id: i64) -> Result<RenduPdf, Box<dyn Error>> {
    let f = db::invoice::get(pool, entity, id)?.ok_or("facture introuvable")?;
    let soc = db::party::get(pool, entity, f.fk_soc.ok_or("facture sans tiers")?)?
        .ok_or("tiers introuvable")?;
    let lignes = db::invoice::lines(pool, entity, id)?;
    let emit = db::company::get(pool, entity)?;

    let vendeur = societe_legale(&emit);
    let client = client_legal(&soc);
    let est_avoir = f.type_ == db::invoice::TYPE_AVOIR;

    // Seule l'identite du vendeur bloque : c'est notre propre fiche, on la
    // complete avant d'emettre. Les identifiants du client, eux, ne bloquent
    // pas — un particulier n'a ni SIRET ni numero de TVA, et refuser sa facture
    // reviendrait a refuser l'essentiel des clients d'un macon. Ils sont
    // signales sur la fiche de la facture (`legal::mentions_client_manquantes`).
    let manquants = legal::champs_manquants(&vendeur, &client);
    if !manquants.is_empty() {
        return Err(format!("facture non conforme : il manque {}", manquants.join(", ")).into());
    }

    let origine = f
        .fk_facture_source
        .and_then(|id| db::invoice::get(pool, entity, id).ok().flatten())
        .map(|o| o.ref_);

    let mut vat: BTreeMap<String, Decimal> = BTreeMap::new();
    for l in &lignes {
        *vat.entry(fmt_qte(l.tva_tx)).or_default() += l.total_tva.unwrap_or_default();
    }

    let doc = pdf::DocumentView {
        kind: if est_avoir {
            pdf::DocKind::CreditNote
        } else {
            pdf::DocKind::Invoice
        },
        title_override: titre_document(f.type_, f.situation_counter),
        reference: f.ref_.clone(),
        date: fmt_date_fr(f.datef.as_deref()),
        due_date: Some(fmt_date_fr(f.date_lim_reglement.as_deref())).filter(|d| !d.is_empty()),
        payment_terms: db::reglement::condition(pool, entity, f.fk_cond_reglement.unwrap_or(0))
            .ok()
            .flatten()
            .map(|c| nettoyer_condition(&c.libelle_facture_fr()))
            .filter(|l| !l.trim().is_empty()),
        deposit: None,
        valid_until: None,
        customer_ref: f.ref_client.clone(),
        service_date: lignes
            .iter()
            .find_map(|l| l.date_start.as_deref())
            .map(|d| fmt_date_fr(Some(d)))
            .filter(|d| !d.is_empty()),
        bank: rib_du_document(
            pool,
            entity,
            db::invoice::compte_bancaire(pool, entity, id)
                .ok()
                .flatten(),
            &emit,
        ),
        cgv: emit.cgv.clone(),
        seller: vendeur_pdf(&emit),
        customer: client_pdf(&soc),
        lines: lignes_facture(&lignes, &db::dict::unites_libelle(pool).unwrap_or_default()),
        total_ht: fmt_eur(f.total_ht),
        total_vat: fmt_eur(f.total_tva),
        total_ttc: fmt_eur(f.total_ttc),
        vat_breakdown: vat
            .into_iter()
            .map(|(t, m)| (format!("{t} %"), fmt_eur(Some(m))))
            .collect(),
        mentions: legal::mentions(
            if est_avoir {
                legal::DocType::Avoir
            } else {
                legal::DocType::Facture
            },
            &vendeur,
            &client,
            &conditions_legales(&emit, origine.as_deref()),
        ),
        notes: f.note_public.clone().filter(|n| !n.trim().is_empty()),
        margins: marges_pdf(pool),
    };

    let modele = modele_pdf(pool, entity, db::pdf_modele::Objet::Facture)?;
    let r = pdf::render_avec_modele(&doc, modele)?;
    Ok(RenduPdf {
        bytes: r.bytes,
        pages: r.pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une chaine vide n'est pas un identifiant : sans ce garde-fou, tout
    /// particulier de la base de production passait pour un professionnel.
    #[test]
    fn une_chaine_vide_ne_fait_pas_un_professionnel() {
        let p = db::party::Party {
            nom: "M DURAND".into(),
            siret: Some(String::new()),
            tva_intra: Some("   ".into()),
            ..Default::default()
        };
        assert!(!client_legal(&p).est_professionnel);
    }

    #[test]
    fn un_vrai_siret_fait_un_professionnel() {
        let p = db::party::Party {
            nom: "SARL TERRA".into(),
            siret: Some("12345678900012".into()),
            ..Default::default()
        };
        assert!(client_legal(&p).est_professionnel);
    }
}
