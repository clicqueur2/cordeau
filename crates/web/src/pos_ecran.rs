//! Écran de caisse (point de vente) : catalogue, panier, encaissement.
//!
//! Le moteur est dans `pos` (panier, arrondis, TVA par ligne), la persistance
//! dans [`db::pos_ventes`]. Ce fichier ne fait que l'écran : aucune règle de
//! calcul, aucune écriture SQL directe.
//!
//! # Où vit le panier, et pourquoi
//!
//! Le panier vit **en mémoire dans ce module** : [`Etat`], une variable
//! statique unique au serveur, qui tient un panier par entité et le dernier
//! message à afficher. Deux raisons, dans cet ordre :
//!
//! 1. `crate::App` est défini dans `main.rs` et n'a pas de champ pour un panier.
//!    Lui en ajouter un obligerait à toucher le routeur, hors du périmètre de ce
//!    fichier ; une statique suffit et ne change rien à la logique de vente.
//! 2. Il n'y a pas de JavaScript : l'état du panier doit bien vivre quelque
//!    part entre deux requêtes HTTP, et le serveur est le seul endroit.
//!
//! Conséquences assumées, à connaître avant de s'en servir :
//!
//! - le panier est **par entité, pas par caissier** : deux postes de caisse de
//!   la même société se partageraient le même panier. Une caisse de commerce
//!   physique a un seul tiroir, c'est le cas visé ;
//! - il **disparaît au redémarrage** du serveur (comme un ticket en cours de
//!   frappe) — la vente, elle, est en base dès qu'elle est encaissée ;
//! - le message de confirmation (`/caisse?ok=vente`) est mémorisé de la même
//!   façon : la signature imposée de [`page`] n'extrait pas la query string,
//!   donc on ne peut pas lire `?ok=`. Le message est **consommé** au premier
//!   affichage.
//!
//! # Deux chemins d'encaissement
//!
//! - **panier** : « + Ajouter » dans le catalogue (POST `/caisse/ajouter`),
//!   puis « Encaisser le panier » (POST `/caisse/encaisser` sans `produit`) ;
//! - **vente directe** : un seul formulaire — article, quantité, moyen,
//!   montant reçu — qui poste sur la même route avec `produit` : le panier
//!   n'est pas touché, la vente part en une ligne.
//!
//! # Le client de passage
//!
//! [`db::pos_ventes::enregistrer`] **refuse** une vente sans tiers : Dolibarr
//! n'a pas de facture sans client. La caisse nomme donc son « client
//! comptoir » (voir [`tiers_comptoir`]) : constante `DOLIBARR_RS_TIERS_COMPTOIR`,
//! sinon un tiers existant qui évoque un client de passage, sinon création à la
//! première vente.
//!
//! # Routes à poser
//!
//! | route | méthode | fonction |
//! |---|---|---|
//! | `/caisse` | GET | [`page`] |
//! | `/caisse/encaisser` | POST | [`encaisser`] |
//! | `/caisse/ajouter` | POST | [`ajouter`] (bouton « + » du catalogue) |
//! | `/caisse/vider` | POST | [`vider`] (bouton « Vider le panier ») |
//!
//! # Ce qui n'est pas fait ici
//!
//! Pas de ticket imprimé (voir `pos::ticket_texte`), pas de clôture de caisse
//! (voir `pos::Session::cloturer`), pas de remise au comptoir : l'écran
//! encaisse ce qui est au panier, au prix de la fiche produit.

// Le fichier n'est pas encore branché sur le routeur de `main.rs` : sans ça,
// chaque fonction publique serait signalée comme morte.

use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard, OnceLock};

use axum::extract::State;
use maud::{html, Markup};
use rust_decimal::Decimal;

use db::dict::Entree as ModePaiement;
use db::product::Product;

/// Constante Dolibarr qui peut désigner le client de passage (`llx_const`).
const CLE_TIERS_COMPTOIR: &str = "DOLIBARR_RS_TIERS_COMPTOIR";

/// Nom du tiers créé à la première vente, faute de client de passage.
const NOM_COMPTOIR: &str = "Client comptoir";

/// Mots qui désignent un client de passage dans une base existante.
const MOTS_COMPTOIR: &[&str] = &["comptoir", "passage", "anonyme", "divers"];

/// Largeur du catalogue affiché : au-delà, on ne vend plus, on consulte.
const CATALOGUE_MAX: u32 = 200;

/// Taille de la page de clients lue pour retrouver le client de passage.
const CLIENTS_MAX: u32 = 500;

// ---------------------------------------------------------------------------
// État de caisse (en mémoire)
// ---------------------------------------------------------------------------

/// Un panier par entité, plus le message à afficher une seule fois.
#[derive(Default)]
struct Etat {
    paniers: HashMap<i64, pos::Panier>,
    messages: HashMap<i64, String>,
}

/// L'état partagé par toutes les requêtes du serveur.
///
/// Le verrou n'est **jamais** tenu pendant une requête : on copie ce qu'il faut
/// puis on le relâche, sinon deux caisses se bloqueraient l'une l'autre.
fn etat() -> MutexGuard<'static, Etat> {
    static ETAT: OnceLock<Mutex<Etat>> = OnceLock::new();
    ETAT.get_or_init(|| Mutex::new(Etat::default()))
        .lock()
        .unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

/// Le panier en cours d'une entité (vide s'il n'y en a pas).
fn panier_de(entity: i64) -> pos::Panier {
    etat().paniers.get(&entity).cloned().unwrap_or_default()
}

/// Remplace le panier d'une entité.
fn poser_panier(entity: i64, panier: pos::Panier) {
    etat().paniers.insert(entity, panier);
}

/// Oublie le panier d'une entité (vente faite, ou panier vidé).
fn vider_panier(entity: i64) {
    etat().paniers.remove(&entity);
}

/// Dépose le message que la prochaine page affichera.
fn annoncer(entity: i64, texte: String) {
    etat().messages.insert(entity, texte);
}

/// Retire le message en attente : il ne s'affiche qu'une fois.
fn message_en_attente(entity: i64) -> Option<String> {
    etat().messages.remove(&entity)
}

// ---------------------------------------------------------------------------
// Lecture des saisies
// ---------------------------------------------------------------------------

/// Lit un nombre saisi en français (« 1 500,50 », « 12,5 », « 2 »).
///
/// `defaut` sert quand le champ est vide ; `None` veut dire « champ
/// obligatoire ». On passe par [`crate::pages::Sorte::Nombre`], qui est la
/// lecture française du dépôt (virgule décimale, espaces de milliers).
fn nombre(
    cle: &str,
    f: &HashMap<String, String>,
    defaut: Option<Decimal>,
) -> Result<Decimal, Box<dyn Error>> {
    let brut = f.get(cle).map(|v| v.trim()).unwrap_or("");
    if brut.is_empty() {
        return match defaut {
            Some(valeur) => Ok(valeur),
            None => Err(format!("« {cle} » est obligatoire").into()),
        };
    }
    let normalise = crate::pages::Sorte::Nombre
        .normaliser(brut)
        .unwrap_or_else(|| brut.to_string());
    Decimal::from_str(&normalise).map_err(|_| -> Box<dyn Error> {
        format!("« {cle} » n'est pas un nombre : « {brut} » (exemple : 2 ou 12,50)").into()
    })
}

// ---------------------------------------------------------------------------
// Panier : du produit à la ligne
// ---------------------------------------------------------------------------

/// Libellé d'un produit à l'écran : la référence prend le relais si le
/// libellé est vide, sinon une ligne de panier serait anonyme.
fn libelle_produit(produit: &Product) -> String {
    if produit.label.trim().is_empty() {
        produit.ref_.clone()
    } else {
        produit.label.clone()
    }
}

/// Prix de vente TTC, tel qu'il doit s'afficher au comptoir.
fn prix_affiche(produit: &Product) -> String {
    let ttc = produit.price_ttc.or_else(|| {
        let ht = produit.price?;
        let taux = produit.tva_tx.unwrap_or_default() / Decimal::from(100);
        Some(money::round_money(ht * (Decimal::ONE + taux)))
    });
    match ttc {
        Some(valeur) => format!("{} TTC", crate::ui::euros(valeur)),
        None => "prix à renseigner".to_string(),
    }
}

/// Une ligne de panier construite depuis la fiche produit lue en base.
///
/// `pos::Ligne` ne connaît que le prix **HT** ; la fiche produit, elle, peut
/// porter un prix TTC (`price_base_type`). On ramène donc le prix à du HT
/// quand c'est un TTC, sinon la TVA serait comptée deux fois sur la facture.
fn ligne_du_produit(produit: &Product, quantite: Decimal) -> Result<pos::Ligne, Box<dyn Error>> {
    let taux = produit.tva_tx.unwrap_or_default();
    let brut = produit
        .price
        .or(produit.price_ttc)
        .ok_or_else(|| -> Box<dyn Error> {
            format!("le produit {} n'a pas de prix à vendre", produit.ref_).into()
        })?;
    let prix_ht = if produit.price_base_type.eq_ignore_ascii_case("TTC") {
        let cent = Decimal::from(100);
        money::round_unit_price(brut * cent / (cent + taux))
    } else {
        brut
    };
    Ok(pos::Ligne::new(
        produit.ref_.clone(),
        libelle_produit(produit),
        quantite,
        prix_ht,
        taux,
    )?)
}

/// Ajoute un produit au panier de l'entité.
fn ajouter_au_panier(
    app: &crate::App,
    entity: i64,
    f: &HashMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    let brut = f.get("produit").map(|v| v.trim()).unwrap_or("");
    let id: i64 = brut
        .parse()
        .map_err(|_| -> Box<dyn Error> { format!("article illisible : « {brut} »").into() })?;
    let produit = db::product::get(&app.pool, entity, id)?.ok_or_else(|| -> Box<dyn Error> {
        format!("article {id} introuvable dans cette société").into()
    })?;
    let quantite = nombre("qty", f, Some(Decimal::ONE))?;
    let ligne = ligne_du_produit(&produit, quantite)?;
    let mut panier = panier_de(entity);
    // `ajouter` rend `true` sur doublon de référence : deux prix différents pour
    // le même article restent deux lignes, c'est voulu par le moteur.
    panier.ajouter(ligne)?;
    poser_panier(entity, panier);
    Ok(())
}

// ---------------------------------------------------------------------------
// Le client de passage
// ---------------------------------------------------------------------------

/// Le tiers au nom duquel les ventes de caisse sont émises.
///
/// Ordre de résolution, du plus explicite au plus automatique :
///
/// 1. la constante Dolibarr `DOLIBARR_RS_TIERS_COMPTOIR` (identifiant posé à la
///    main, la voie propre pour une installation existante) ;
/// 2. un tiers client dont le nom évoque un client de passage (« comptoir »,
///    « anonyme », « divers », « passage ») ;
/// 3. **si `creer` est vrai seulement** — c'est-à-dire au moment d'encaisser —,
///    création d'un « Client comptoir ». L'affichage d'une page ne crée jamais
///    de donnée : seule une vente réelle en écrit.
fn tiers_comptoir(
    app: &crate::App,
    entity: i64,
    creer: bool,
) -> Result<Option<(i64, String)>, Box<dyn Error>> {
    if let Some(constante) = db::company::get_const(&app.pool, CLE_TIERS_COMPTOIR)? {
        if let Ok(id) = constante.trim().parse::<i64>() {
            if let Some(tiers) = db::party::get(&app.pool, entity, id)? {
                return Ok(Some((tiers.rowid, tiers.nom)));
            }
        }
    }

    let clients = db::party::list_filtre(&app.pool, entity, "client IN (1,3)", CLIENTS_MAX)?;
    let trouve = clients.iter().find(|tiers| {
        // `normalise_moyen` est le normalisateur du dépôt : minuscules et
        // accents retirés (« Comptoir du coin » -> « comptoir du coin »).
        let nom = pos::normalise_moyen(&tiers.nom);
        MOTS_COMPTOIR.iter().any(|mot| nom.contains(mot))
    });
    if let Some(tiers) = trouve {
        return Ok(Some((tiers.rowid, tiers.nom.clone())));
    }

    if !creer {
        return Ok(None);
    }
    let id = db::party::create(
        &app.pool,
        entity,
        &db::party::NewParty::client(NOM_COMPTOIR),
    )?;
    Ok(Some((id, NOM_COMPTOIR.to_string())))
}

// ---------------------------------------------------------------------------
// Fragments de formulaire
// ---------------------------------------------------------------------------

/// Un champ de formulaire : intitulé au-dessus du contrôle.
fn champ(intitule: &str, controle: Markup) -> Markup {
    html! {
        label style="display:block" {
            span style="display:block;font-size:12.5px;color:#78736c;margin-bottom:4px" {
                (intitule)
            }
            (controle)
        }
    }
}

/// Libellé français d'un moyen de paiement.
///
/// Le dictionnaire Dolibarr est livré en anglais (« Cash », « Credit card ») :
/// une caisse parle au caissier dans sa langue. Le libellé de la base sert de
/// repli pour un code inconnu.
fn libelle_moyen(mode: &ModePaiement) -> String {
    match mode.code.to_ascii_uppercase().as_str() {
        "ESP" | "LIQ" => "Espèces".to_string(),
        "CB" | "CRD" => "Carte bancaire".to_string(),
        "CHQ" => "Chèque".to_string(),
        "VIR" => "Virement".to_string(),
        "PRE" => "Prélèvement".to_string(),
        _ => {
            if mode.libelle.trim().is_empty() {
                mode.code.clone()
            } else {
                mode.libelle.clone()
            }
        }
    }
}

/// Le sélecteur de moyen de paiement, lu dans `llx_c_paiement`.
///
/// La valeur envoyée est l'**identifiant** du dictionnaire : `pos_ventes`
/// résout un identifiant numérique lui-même, ce qui évite de dépendre des
/// codes — `ESP` manque à certaines installations, `LIQ` le remplace.
fn choix_moyen(modes: &[ModePaiement]) -> Markup {
    html! {
        select name="moyen" aria-label="Moyen de paiement"
               style="width:100%;padding:12px 14px;font-size:16px" {
            @if modes.is_empty() {
                // Dictionnaire vide : les trois moyens qu'une caisse utilise
                // vraiment, pour que l'écran reste utilisable.
                option value="especes" {(crate::langue::tr("Espèces")) }
                option value="cb" {(crate::langue::tr("Carte bancaire")) }
                option value="cheque" {(crate::langue::tr("Chèque")) }
            } @else {
                @for mode in modes {
                    option value=(mode.id) { (libelle_moyen(mode)) }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Écran
// ---------------------------------------------------------------------------

/// L'écran de caisse : total du jour, panier en cours, catalogue.
pub async fn page(State(app): State<crate::App>) -> crate::Res<Markup> {
    let entity = crate::entite::courante();
    let jour = db::aujourdhui(&app.pool)?;
    let produits = db::product::list(&app.pool, entity, CATALOGUE_MAX, 0)?;
    let (total, ventes) = db::pos_ventes::total_du_jour(&app.pool, entity, &jour)?;
    let modes = db::dict::modes_paiement(&app.pool)?;
    let comptoir = tiers_comptoir(&app, entity, false)?;
    let panier = panier_de(entity);
    let message = message_en_attente(entity);

    // Un produit caché ou non proposé à la vente n'a rien à faire au comptoir.
    let vendables: Vec<&Product> = produits
        .iter()
        .filter(|produit| produit.on_sell() && produit.hidden == 0)
        .collect();

    let note_ventes = format!("{ventes} vente(s) le {jour}");
    let note_panier = format!("{} ligne(s)", panier.nombre_lignes());
    let texte_comptoir = match &comptoir {
        Some((_, nom)) => format!("ventes au nom de « {nom} »"),
        None => "aucun client de passage : la première vente en créera un".to_string(),
    };

    let contenu = html! {
        h1 {(crate::langue::tr("Caisse")) }
        (etat_scelle_pour(&app).await?)
        p .sous-titre {
            "Encaissement au comptoir. Aujourd'hui : "
            strong { (ventes.to_string()) }
            " vente(s), "
            strong { (crate::ui::euros(total)) }
            " encaissés."
        }
        @if let Some(texte) = &message {
            (crate::ui::flash(texte))
        }
        div .cartes {
            (crate::ui::carte(
                "Encaissé aujourd'hui",
                &crate::ui::euros(total),
                Some(note_ventes.as_str()),
                false,
            ))
            (crate::ui::carte(
                "Panier en cours",
                &crate::ui::euros(panier.total_ttc()),
                Some(note_panier.as_str()),
                false,
            ))
        }
        p style="color:#78736c;font-size:13.5px;margin:12px 0 0" {
            "Client de passage : " (texte_comptoir)
        }

        h2 {(crate::langue::tr("Panier en cours")) }
        @if panier.est_vide() {
            (crate::ui::vide(
                "Panier vide : ajoutez un article depuis le catalogue, ou passez par la vente directe.",
                None,
            ))
        } @else {
            table {
                thead {
                    tr {
                        th {(crate::langue::tr("Article")) }
                        th {(crate::langue::tr("Réf.")) }
                        th .num {(crate::langue::tr("Qté")) }
                        th .num {(crate::langue::tr("Total TTC")) }
                    }
                }
                tbody {
                    @for (ligne, totaux) in panier.lignes().iter().zip(panier.detail()) {
                        tr {
                            td { (ligne.libelle) }
                            td .ref { (ligne.reference) }
                            td .num { (ligne.quantite.normalize().to_string()) }
                            td .num { (crate::ui::euros(totaux.ttc)) }
                        }
                    }
                }
                tfoot {
                    tr {
                        td colspan="3" {(crate::langue::tr("Total à payer")) }
                        td .num { (crate::ui::euros(panier.total_ttc())) }
                    }
                }
            }
            form method="post" action="/caisse/encaisser"
                 style="margin-top:16px;display:flex;gap:14px;flex-wrap:wrap;align-items:end" {
                (champ("Moyen de paiement", choix_moyen(&modes)))
                (champ("Montant reçu (facultatif)", html! {
                    input type="text" name="paye" inputmode="decimal"
                          placeholder="vide = total TTC"
                          style="padding:14px 16px;font-size:16px";
                }))
                div {
                    button type="submit" style="padding:18px 22px;font-size:16px" {(crate::langue::tr("Encaisser le panier"))
                    }
                }
            }
            form method="post" action="/caisse/vider" class="haut" {
                button type="submit"
                       style="background:#fff;color:#1c1b19;border:1px solid #e6e2dc;\
                              padding:12px 18px;font-size:15px" {(crate::langue::tr("Vider le panier"))
                }
            }
        }

        h2 {(crate::langue::tr("Catalogue")) }
        @if vendables.is_empty() {
            (crate::ui::vide(
                "Aucun produit proposé à la vente dans cette société.",
                Some(("/produits/nouveau", "Créer un produit")),
            ))
        } @else {
            div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:14px" {
                @for produit in vendables.iter().copied() {
                    div .carte {
                        div style="display:flex;justify-content:space-between;gap:10px;align-items:baseline" {
                            strong { (libelle_produit(produit)) }
                            span .ref style="color:#78736c;font-size:12.5px" { (produit.ref_) }
                        }
                        div .note { (prix_affiche(produit)) }
                        form method="post" action="/caisse/ajouter" style="margin-top:12px" {
                            input type="hidden" name="produit" value=(produit.rowid);
                            button type="submit"
                                   style="padding:18px 22px;font-size:16px;width:100%" {(crate::langue::tr("+ Ajouter au panier"))
                            }
                        }
                    }
                }
            }
        }

        h2 {(crate::langue::tr("Vente directe")) }
        p class="aide bloc" {
            "Un article, une quantité, un encaissement : la vente part en une ligne, \
             sans passer par le panier."
        }
        @if vendables.is_empty() {
            (crate::ui::vide("Aucun article encaissable pour l'instant.", None))
        } @else {
            form method="post" action="/caisse/encaisser"
                 style="display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));\
                        gap:14px;align-items:end;max-width:900px" {
                (champ("Article", html! {
                    select name="produit" style="width:100%;padding:12px 14px;font-size:16px" {
                        @for produit in vendables.iter().copied() {
                            option value=(produit.rowid) {
                                (produit.ref_) " — " (libelle_produit(produit))
                            }
                        }
                    }
                }))
                (champ("Quantité", html! {
                    input type="text" name="qty" value="1" inputmode="decimal"
                          style="width:100%;padding:12px 14px;font-size:16px";
                }))
                (champ("Moyen de paiement", choix_moyen(&modes)))
                (champ("Montant reçu (facultatif)", html! {
                    input type="text" name="paye" inputmode="decimal"
                          placeholder="vide = total TTC"
                          style="width:100%;padding:12px 14px;font-size:16px";
                }))
                div {
                    button type="submit" style="padding:18px 22px;font-size:16px;width:100%" {(crate::langue::tr("Encaisser"))
                    }
                }
            }
        }
    };

    // Le module « Produits » porte l'écran tant qu'aucune entrée « Caisse »
    // n'existe au menu : la barre de gauche reste celle du catalogue.
    Ok(crate::ui::page("produits", "caisse", "Caisse", contenu))
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Encaisse : une ligne (`produit`, `qty`) ou le panier en cours (`produit`
/// absent), puis revient sur `/caisse?ok=vente`.
///
/// `moyen` accepte un identifiant du dictionnaire ou un libellé (« espèces »,
/// « cb »...) : c'est [`db::pos_ventes::enregistrer`] qui le résout.
/// `paye` vide vaut le TTC du panier ; un montant supérieur laisse un rendu,
/// un montant inférieur laisse la facture en brouillon (vente à crédit).
pub async fn encaisser(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> crate::Res<axum::response::Redirect> {
    Ok(vente(&app, &f)?)
}

/// Le corps de l'encaissement, hors extraction HTTP.
fn vente(
    app: &crate::App,
    f: &HashMap<String, String>,
) -> Result<axum::response::Redirect, Box<dyn Error>> {
    let entity = crate::entite::courante();
    let jour = db::aujourdhui(&app.pool)?;

    let article = f.get("produit").map(|v| v.trim()).filter(|v| !v.is_empty());

    // `depuis_panier` : la vente consomme le panier, il faudra donc le vider
    // quand elle aura réussi (une vente refusée ne doit rien perdre).
    let (panier, depuis_panier) = match article {
        Some(brut) => {
            let id: i64 = brut.parse().map_err(|_| -> Box<dyn Error> {
                format!("article illisible : « {brut} »").into()
            })?;
            let produit =
                db::product::get(&app.pool, entity, id)?.ok_or_else(|| -> Box<dyn Error> {
                    format!("article {id} introuvable dans cette société").into()
                })?;
            let quantite = nombre("qty", f, Some(Decimal::ONE))?;
            let mut seul = pos::Panier::new();
            seul.ajouter(ligne_du_produit(&produit, quantite)?)?;
            (seul, false)
        }
        None => (panier_de(entity), true),
    };

    if panier.est_vide() {
        return Err("le panier est vide : ajoutez un article avant d'encaisser".into());
    }

    let ttc = panier.total_ttc();
    let paye = match f.get("paye").map(|v| v.trim()).filter(|v| !v.is_empty()) {
        Some(_) => nombre("paye", f, None)?,
        None => ttc,
    };
    if paye <= Decimal::ZERO {
        return Err("le montant encaissé doit être supérieur à zéro".into());
    }
    let moyen = f
        .get("moyen")
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| -> Box<dyn Error> { "choisissez un moyen de paiement".into() })?;

    let (tiers, nom_tiers) = tiers_comptoir(app, entity, true)?
        .ok_or_else(|| -> Box<dyn Error> { "aucun client de passage pour la vente".into() })?;

    let facture =
        db::pos_ventes::enregistrer(&app.pool, entity, Some(tiers), &panier, moyen, paye, &jour)?;

    if depuis_panier {
        vider_panier(entity);
    }

    let reference = db::invoice::get(&app.pool, entity, facture)?
        .map(|piece| piece.ref_)
        .unwrap_or_else(|| format!("n° {facture}"));

    let mut texte = format!(
        "Vente enregistrée : facture {reference} pour {nom_tiers}, {} encaissés.",
        crate::ui::euros(ttc)
    );
    if paye > ttc {
        texte.push_str(&format!(" Rendu : {}.", crate::ui::euros(paye - ttc)));
    } else if paye < ttc {
        texte.push_str(&format!(
            " Reste dû : {} — la facture reste en brouillon.",
            crate::ui::euros(ttc - paye)
        ));
    }
    annoncer(entity, texte);

    Ok(axum::response::Redirect::to("/caisse?ok=vente"))
}

/// Ajoute un article au panier et revient sur l'écran de caisse.
pub async fn ajouter(
    State(app): State<crate::App>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> crate::Res<axum::response::Redirect> {
    ajouter_au_panier(&app, crate::entite::courante(), &f)?;
    Ok(axum::response::Redirect::to("/caisse?ok=ajoute"))
}

/// Vide le panier en cours (une ligne de trop se retire en vidant, puis en
/// rajoutant : il n'y a pas de JavaScript pour décrémenter une ligne).
pub async fn vider(State(_app): State<crate::App>) -> crate::Res<axum::response::Redirect> {
    vider_panier(crate::entite::courante());
    Ok(axum::response::Redirect::to("/caisse?ok=vide"))
}

// ---------------------------------------------------------------------------
// Conformite : scelle de la journee et archive verifiable.
//
// L'article 286-I-3° bis du CGI demande l'inalterabilite, la conservation et
// l'archivage des donnees de caisse. Ces ecrans ne certifient rien : ils
// rendent la chose visible et utilisable — sceller, constater, archiver.
// ---------------------------------------------------------------------------

/// Etat du scelle du jour, affiche a cote du panier.
pub async fn etat_scelle_pour(app: &crate::App) -> crate::Res<maud::Markup> {
    let jour = db::caisse_scelle::aujourd_hui_iso();
    let scelle = db::caisse_scelle::calculer(&app.pool, crate::entite::courante(), &jour)?;

    Ok(match scelle.intact() {
        None => maud::html! {
            div .carte {
                div .libelle { "Scelle du " (jour) }
                div .valeur {(crate::langue::tr("non pose")) }
                div .note { (scelle.ventes.to_string()) " vente(s) a sceller." }
                form method="post" action="/caisse/sceller" class="haut" {
                    button type="submit" {(crate::langue::tr("Sceller la journee")) }
                }
            }
        },
        Some(true) => maud::html! {
            div .carte {
                div .libelle { "Scelle du " (jour) }
                div .valeur style="color:#2f6b45" { "intact" }
                div .note {
                    (scelle.ventes.to_string()) " vente(s) - "
                    (crate::ui::euros(scelle.total)) " - empreinte "
                    code { (scelle.empreinte.chars().take(12).collect::<String>()) }
                }
                a .bouton.secondaire.haut href={ "/caisse/archive?date=" (jour) } {(crate::langue::tr("Telecharger l'archive"))
                }
            }
        },
        Some(false) => maud::html! {
            div .carte.alerte {
                div .libelle { "Scelle du " (jour) }
                div .valeur {(crate::langue::tr("FALSIFIE")) }
                div .note {
                    "Les ventes ne correspondent plus au scelle pose : une ligne a ete "
                    "modifiee, ajoutee ou supprimee."
                }
                a .bouton.secondaire.haut href={ "/caisse/archive?date=" (jour) } {(crate::langue::tr("Voir l'archive"))
                }
            }
        },
    })
}

/// Pose le scelle du jour, ou relit celui qui existe.
pub async fn sceller(State(app): State<crate::App>) -> crate::Res<axum::response::Redirect> {
    let jour = db::caisse_scelle::aujourd_hui_iso();
    let scelle = db::caisse_scelle::sceller(&app.pool, crate::entite::courante(), &jour)?;
    let code = if scelle.intact() == Some(false) {
        "scelle_desaccord"
    } else {
        "scelle"
    };
    Ok(axum::response::Redirect::to(&format!("/caisse?ok={code}")))
}

/// L'archive verifiable, en piece jointe texte.
pub async fn archive(
    State(app): State<crate::App>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> crate::Res<axum::response::Response> {
    use axum::response::IntoResponse;
    let jour = q
        .get("date")
        .cloned()
        .unwrap_or_else(db::caisse_scelle::aujourd_hui_iso);
    let texte = db::caisse_scelle::archive(&app.pool, crate::entite::courante(), &jour)?;
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "text/plain; charset=utf-8",
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"caisse-archive.txt\"",
            ),
        ],
        texte,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    //! Ce qui se vérifie sans base : lecture des saisies, prix HT/TTC, panier
    //! en mémoire et contenu du sélecteur de moyen de paiement.

    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("décimal de test valide")
    }

    fn saisie(paires: &[(&str, &str)]) -> HashMap<String, String> {
        paires
            .iter()
            .map(|(cle, valeur)| ((*cle).to_string(), (*valeur).to_string()))
            .collect()
    }

    fn produit(prix: &str, base: &str, taux: &str) -> Product {
        Product {
            rowid: 7,
            ref_: "ART-7".to_string(),
            label: "Article de test".to_string(),
            price: Some(d(prix)),
            price_base_type: base.to_string(),
            tva_tx: Some(d(taux)),
            tosell: 1,
            ..Default::default()
        }
    }

    #[test]
    fn les_nombres_se_lisent_en_francais() {
        let f = saisie(&[("qty", "1 500,50")]);
        assert_eq!(nombre("qty", &f, None).expect("nombre lu"), d("1500.50"));

        // Champ vide : la valeur par défaut prend le relais, sinon c'est une
        // erreur explicite (un champ obligatoire vide ne vaut pas zéro).
        let vide = saisie(&[("paye", "   ")]);
        assert_eq!(
            nombre("paye", &vide, Some(Decimal::ONE)).expect("défaut"),
            Decimal::ONE
        );
        assert!(nombre("paye", &vide, None).is_err());

        // Une saisie qui n'est pas un nombre est refusée, jamais interprétée
        // comme zéro : une caisse ne doit pas encaisser un montant inventé.
        let faux = saisie(&[("paye", "deux")]);
        let erreur = nombre("paye", &faux, None).expect_err("refusé");
        assert!(erreur.to_string().contains("paye"));
    }

    #[test]
    fn un_prix_ttc_est_ramene_en_ht() {
        // Fiche produit à 120,00 TTC à 20 % : la ligne doit porter 100,00 HT,
        // sinon la TVA serait ajoutée une seconde fois sur la facture.
        let ligne = ligne_du_produit(&produit("120.00", "TTC", "20"), Decimal::ONE)
            .expect("ligne construite");
        assert_eq!(ligne.prix_unitaire_ht, d("100.00"));

        let ligne = ligne_du_produit(&produit("100.00", "HT", "20"), Decimal::ONE)
            .expect("ligne construite");
        assert_eq!(ligne.prix_unitaire_ht, d("100.00"));

        // Sans prix, la ligne est refusée avec le nom du produit en clair.
        let mut sans_prix = produit("100.00", "HT", "20");
        sans_prix.price = None;
        sans_prix.price_ttc = None;
        let erreur = ligne_du_produit(&sans_prix, Decimal::ONE).expect_err("refusé");
        assert!(erreur.to_string().contains("ART-7"));
    }

    #[test]
    fn le_panier_est_propre_a_chaque_entite() {
        // Une entité de test, pour ne pas marcher sur l'état réel du serveur.
        let entite = 9_001;
        vider_panier(entite);
        assert!(panier_de(entite).est_vide());

        let mut panier = panier_de(entite);
        panier
            .ajouter(
                pos::Ligne::new("ART-7", "Article", Decimal::ONE, d("100.00"), d("20"))
                    .expect("ligne valide"),
            )
            .expect("ajout");
        poser_panier(entite, panier);

        assert_eq!(panier_de(entite).nombre_lignes(), 1);
        assert_eq!(panier_de(entite).total_ttc(), d("120.00"));
        assert!(panier_de(entite + 1).est_vide(), "un panier par entité");

        vider_panier(entite);
        assert!(panier_de(entite).est_vide());
    }

    #[test]
    fn le_moyen_de_paiement_envoye_est_lidentifiant_du_dictionnaire() {
        // La valeur postée est l'identifiant de `llx_c_paiement` : c'est ce que
        // `pos_ventes` sait résoudre, quel que soit le code de l'installation.
        let modes = vec![
            ModePaiement {
                id: 4,
                code: "LIQ".to_string(),
                libelle: "Cash".to_string(),
                actif: true,
            },
            ModePaiement {
                id: 6,
                code: "CB".to_string(),
                libelle: "Credit card".to_string(),
                actif: true,
            },
        ];
        let rendu = choix_moyen(&modes).into_string();
        assert!(
            rendu.contains("value=\"4\""),
            "identifiant attendu : {rendu}"
        );
        assert!(rendu.contains("Espèces"), "libellé français attendu");
        assert!(rendu.contains("Carte bancaire"));

        // Dictionnaire vide : l'écran reste utilisable avec les trois moyens
        // qu'une caisse utilise vraiment.
        let secours = choix_moyen(&[]).into_string();
        assert!(secours.contains("value=\"especes\""));
        assert!(secours.contains("value=\"cb\""));
        assert!(secours.contains("value=\"cheque\""));
    }
}
