//! Serveur MCP (Model Context Protocol) sur l'entree/sortie standard.
//!
//! Un agent (Claude, opencode, n8n) lance ce binaire et pilote directement la
//! base Dolibarr : lire les tiers, creer un devis, le valider, facturer une
//! situation, encaisser. Aucun HTTP, aucune session : le JSON-RPC passe par
//! stdin/stdout, comme le veut le protocole.
//!
//! Les montants sont des chaines decimales, jamais des flottants.

use std::error::Error;
use std::io::{self, BufRead, Write};

use std::str::FromStr;

use rust_decimal::Decimal;
use serde_json::{json, Value};

const ENTITY: i64 = 1;

fn main() {
    let dsn = std::env::var("DOLIBARR_DSN")
        .unwrap_or_else(|_| "mysql://root:root@127.0.0.1:3307/dolibarr".to_string());
    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion MySQL impossible : {e}");
            std::process::exit(1);
        }
    };

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for ligne in stdin.lock().lines() {
        let Ok(ligne) = ligne else { break };
        if ligne.trim().is_empty() {
            continue;
        }
        let requete: Value = match serde_json::from_str(&ligne) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(
                    stdout,
                    "{}",
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":e.to_string()}})
                );
                continue;
            }
        };
        let id = requete.get("id").cloned().unwrap_or(Value::Null);
        let methode = requete.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // Les notifications n'ont pas de reponse.
        if id.is_null() && methode.starts_with("notifications/") {
            continue;
        }

        let reponse = match methode {
            "initialize" => Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "dolibarr-mcp", "version": env!("CARGO_PKG_VERSION") }
            })),
            "tools/list" => Ok(json!({ "tools": outils() })),
            "tools/call" => appeler_outil(&pool, &requete).map_err(|e| e.to_string()),
            autre => Err(format!("methode inconnue : {autre}")),
        };

        let corps = match reponse {
            Ok(v) => json!({"jsonrpc":"2.0","id":id,"result":v}),
            Err(message) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":message}})
            }
        };
        let _ = writeln!(stdout, "{corps}");
        let _ = stdout.flush();
    }
}

fn outils() -> Value {
    json!([
        outil(
            "lister_tiers",
            "Liste les tiers (clients et fournisseurs).",
            json!({"type":"object","properties":{"limite":{"type":"integer","description":"nombre maximum"}}})
        ),
        outil(
            "creer_tiers",
            "Cree un tiers client.",
            json!({"type":"object","properties":{
                  "nom":{"type":"string"},
                  "email":{"type":"string"},
                  "ville":{"type":"string"}
              },"required":["nom"]})
        ),
        outil(
            "lister_devis",
            "Liste les devis avec leur statut et leurs totaux.",
            json!({"type":"object"})
        ),
        outil(
            "lire_devis",
            "Detaille un devis : lignes, totaux, situations deja facturees.",
            json!({"type":"object","properties":{"id":{"type":"integer"}},"required":["id"]})
        ),
        outil(
            "creer_devis",
            "Cree un devis brouillon pour un tiers.",
            json!({"type":"object","properties":{
                  "tiers":{"type":"integer"},
                  "validite":{"type":"string","description":"AAAA-MM-JJ"},
                  "lignes":{"type":"array","items":{"type":"object","properties":{
                      "label":{"type":"string"},
                      "qty":{"type":"string"},
                      "pu_ht":{"type":"string"},
                      "tva":{"type":"string"}
                  },"required":["label","qty","pu_ht","tva"]}}
              },"required":["tiers","lignes"]})
        ),
        outil(
            "valider_devis",
            "Valide un devis en brouillon.",
            json!({"type":"object","properties":{"id":{"type":"integer"}},"required":["id"]})
        ),
        outil(
            "facturer_situation",
            "Cree une facture de situation (pourcentage du devis).",
            json!({"type":"object","properties":{
                  "devis":{"type":"integer"},
                  "pourcentage":{"type":"string"},
                  "date":{"type":"string"}
              },"required":["devis","pourcentage"]})
        ),
        outil(
            "creer_commande",
            "Cree une commande client depuis un devis valide.",
            json!({"type":"object","properties":{"devis":{"type":"integer"}},"required":["devis"]})
        ),
        outil(
            "valider_commande",
            "Valide une commande client en brouillon.",
            json!({"type":"object","properties":{"commande":{"type":"integer"}},"required":["commande"]})
        ),
        outil(
            "facturer_commande",
            "Cree la facture d'une commande validee.",
            json!({"type":"object","properties":{"commande":{"type":"integer"}},"required":["commande"]})
        ),
        outil(
            "lister_factures",
            "Liste les factures avec le reste du.",
            json!({"type":"object"})
        ),
        outil(
            "lire_facture",
            "Detaille une facture (lignes, regle, reste du).",
            json!({"type":"object","properties":{"id":{"type":"integer"}},"required":["id"]})
        ),
        outil(
            "encaisser_facture",
            "Enregistre un reglement sur une facture.",
            json!({"type":"object","properties":{
                  "facture":{"type":"integer"},
                  "montant":{"type":"string"},
                  "mode":{"type":"string","description":"virement, cheque, especes, carte, prelevement"},
                  "compte":{"type":"integer"}
              },"required":["facture","montant"]})
        ),
        outil(
            "solde_banque",
            "Soldes des comptes bancaires.",
            json!({"type":"object"})
        ),
        outil(
            "carnet_commandes",
            "Le carnet : commandes validees et livrees, et ce qui est livre sans facture.",
            json!({"type":"object"})
        ),
        outil(
            "marge_chantiers",
            "Marge par chantier : facture client moins facture fournisseur.",
            json!({"type":"object"})
        ),
        outil(
            "lister_chantiers",
            "Liste les chantiers avec leur budget et leurs dates.",
            json!({"type":"object","properties":{"limite":{"type":"integer"}}})
        ),
        outil(
            "stock_niveaux",
            "Niveaux de stock par produit et par entrepot.",
            json!({"type":"object"})
        ),
        outil(
            "pdf_devis",
            "Ecrit le PDF d'un devis sur le disque et rend son chemin.",
            json!({"type":"object","properties":{
                  "id":{"type":"integer"},
                  "chemin":{"type":"string","description":"defaut : repertoire temporaire"}
              },"required":["id"]})
        ),
        outil(
            "lister_contrats",
            "Liste les contrats avec leur statut et leurs dates.",
            json!({"type":"object"})
        ),
        outil(
            "creer_contrat",
            "Cree un contrat en brouillon pour un tiers.",
            json!({"type":"object","properties":{
                  "tiers":{"type":"integer"},
                  "debut":{"type":"string","description":"AAAA-MM-JJ"},
                  "fin":{"type":"string","description":"AAAA-MM-JJ, facultatif"}
              },"required":["tiers"]})
        ),
        outil(
            "mouvement_stock",
            "Entre ou sort du stock pour un produit dans un entrepot (type 0 = entree, 1 = sortie).",
            json!({"type":"object","properties":{
                  "produit":{"type":"integer"},
                  "entrepot":{"type":"integer"},
                  "quantite":{"type":"string"},
                  "type":{"type":"integer","description":"0 entree, 1 sortie"},
                  "libelle":{"type":"string"}
              },"required":["produit","entrepot","quantite","type"]})
        ),
        outil(
            "solde_tva",
            "TVA collectee, deductible et solde a payer.",
            json!({"type":"object"})
        ),
    ])
}

fn outil(nom: &str, description: &str, schema: Value) -> Value {
    json!({ "name": nom, "description": description, "inputSchema": schema })
}

fn arg<'a>(corps: &'a Value, nom: &str) -> Option<&'a Value> {
    // `corps` **est** l'objet `arguments` : on le lit directement. La version
    // precedente cherchait `corps["arguments"][nom]`, c'est-a-dire un niveau de
    // trop, et tous les outils retombaient silencieusement sur leurs valeurs
    // par defaut — ou refusaient un argument pourtant fourni.
    corps.get(nom).filter(|v| !v.is_null())
}

fn arg_i64(corps: &Value, nom: &str) -> Result<i64, Box<dyn Error>> {
    let v = arg(corps, nom)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("argument {nom} manquant"))?;
    Ok(v)
}

fn arg_str(corps: &Value, nom: &str) -> Result<String, Box<dyn Error>> {
    let v = arg(corps, nom)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("argument {nom} manquant"))?;
    Ok(v)
}

fn arg_dec(corps: &Value, nom: &str) -> Result<Decimal, Box<dyn Error>> {
    let s = arg_str(corps, nom)?;
    // `money::dec()` fait `.expect()` : un agent qui envoie `"quantite": "deux"`
    // tuait le serveur MCP. Un argument illisible est une erreur, pas un arret.
    Decimal::from_str(s.trim()).map_err(|_| format!("{nom} : « {s} » n'est pas un nombre").into())
}

fn texte(v: Value) -> Value {
    json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] })
}

fn appeler_outil(pool: &db::Pool, requete: &Value) -> Result<Value, Box<dyn Error>> {
    let params = requete.get("params").cloned().unwrap_or(Value::Null);
    let nom = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("params.name manquant")?;
    let corps = params.get("arguments").cloned().unwrap_or(json!({}));

    let valeur: Result<Value, Box<dyn Error>> = match nom {
        "lister_tiers" => {
            let limite = arg(&corps, "limite").and_then(|v| v.as_u64()).unwrap_or(20) as u32;
            Ok(json!(db::party::list(pool, ENTITY, limite, 0)?
                .iter()
                .map(|t| json!({"id": t.rowid, "nom": t.nom, "ville": t.town, "email": t.email}))
                .collect::<Vec<_>>()))
        }
        "creer_tiers" => {
            let nom_tiers = arg_str(&corps, "nom")?;
            let mut nouveau = db::party::NewParty::client(&nom_tiers);
            nouveau.email = arg(&corps, "email")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            nouveau.town = arg(&corps, "ville")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            Ok(json!({ "id": db::party::create(pool, ENTITY, &nouveau)? }))
        }
        "lister_devis" => Ok(json!(db::proposal::list(pool, ENTITY, 50, 0)?
            .iter()
            .map(|d| json!({
                "id": d.rowid, "ref": d.ref_, "tiers": d.fk_soc,
                "statut": d.fk_statut, "total_ttc": d.total_ttc.map(|v| v.normalize().to_string()),
            }))
            .collect::<Vec<_>>())),
        "lire_devis" => {
            let id = arg_i64(&corps, "id")?;
            let d = db::proposal::get(pool, ENTITY, id)?.ok_or("devis introuvable")?;
            let lignes = db::proposal::lines(pool, ENTITY, id)?;
            let situations = db::situation::situations(pool, ENTITY, id)?;
            Ok(json!({
                "id": d.rowid, "ref": d.ref_, "statut": d.fk_statut,
                "total_ht": d.total_ht.map(|v| v.normalize().to_string()),
                "total_ttc": d.total_ttc.map(|v| v.normalize().to_string()),
                "lignes": lignes.iter().map(|l| json!({
                    "label": l.label, "qty": l.qty.map(|v| v.normalize().to_string()),
                    "total_ht": l.total_ht.map(|v| v.normalize().to_string()),
                })).collect::<Vec<_>>(),
                "situations": situations.iter().map(|s| json!({
                    "ref": s.ref_, "numero": s.counter, "finale": s.finale,
                    "pourcentage": s.pourcentage.normalize().to_string(),
                    "total_ttc": s.total_ttc.normalize().to_string(),
                })).collect::<Vec<_>>(),
            }))
        }
        "creer_devis" => {
            let fk_soc = arg_i64(&corps, "tiers")?;
            let lignes = corps
                .get("lignes")
                .and_then(|v| v.as_array())
                .ok_or("argument lignes manquant")?;
            let mut nouveau = db::proposal::NewProposal::new(fk_soc);
            nouveau.fin_validite = arg(&corps, "validite")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            nouveau.lines = lignes
                .iter()
                .map(|l| {
                    db::proposal::NewProposalLine::new(
                        l.get("label").and_then(|v| v.as_str()).unwrap_or("Ligne"),
                        money::dec(l.get("qty").and_then(|v| v.as_str()).unwrap_or("1")),
                        money::dec(l.get("pu_ht").and_then(|v| v.as_str()).unwrap_or("0")),
                        money::dec(l.get("tva").and_then(|v| v.as_str()).unwrap_or("20")),
                    )
                })
                .collect();
            let id = db::proposal::create(pool, ENTITY, &nouveau)?;
            Ok(json!({ "id": id }))
        }
        "valider_devis" => {
            let id = arg_i64(&corps, "id")?;
            db::proposal::validate(pool, ENTITY, id)?;
            Ok(json!({ "id": id, "statut": 1 }))
        }
        "facturer_situation" => {
            let devis = arg_i64(&corps, "devis")?;
            let pct = arg_dec(&corps, "pourcentage")?;
            let date = arg(&corps, "date").and_then(|v| v.as_str());
            let facture = db::situation::creer(pool, ENTITY, devis, pct, date)?;
            Ok(json!({ "facture": facture }))
        }
        "creer_commande" => {
            let devis = arg_i64(&corps, "devis")?;
            Ok(json!({ "commande": db::commande::create_from_proposal(pool, ENTITY, devis)? }))
        }
        "valider_commande" => {
            let commande = arg_i64(&corps, "commande")?;
            db::commande::validate(pool, ENTITY, commande)?;
            Ok(json!({ "commande": commande, "statut": 1 }))
        }
        "facturer_commande" => {
            let commande = arg_i64(&corps, "commande")?;
            Ok(json!({ "facture": db::invoice::create_from_commande(pool, ENTITY, commande)? }))
        }
        "lister_factures" => Ok(json!(db::invoice::list(pool, ENTITY, 50, 0)?
            .iter()
            .map(|f| {
                let reste = db::payment::balance(pool, ENTITY, f.rowid)
                    .ok()
                    .map(|b| b.reste);
                json!({
                    "id": f.rowid, "ref": f.ref_, "type": f.type_,
                    "statut": f.fk_statut, "echeance": f.date_lim_reglement,
                    "total_ttc": f.total_ttc.map(|v| v.normalize().to_string()),
                    "reste_du": reste.map(|v| v.normalize().to_string()),
                })
            })
            .collect::<Vec<_>>())),
        "lire_facture" => {
            let id = arg_i64(&corps, "id")?;
            let f = db::invoice::get(pool, ENTITY, id)?.ok_or("facture introuvable")?;
            let solde = db::payment::balance(pool, ENTITY, id)?;
            Ok(json!({
                "id": f.rowid, "ref": f.ref_, "type": f.type_, "statut": f.fk_statut,
                "total_ttc": f.total_ttc.map(|v| v.normalize().to_string()),
                "regle": solde.regle.normalize().to_string(),
                "reste_du": solde.reste.normalize().to_string(),
            }))
        }
        "encaisser_facture" => {
            let facture = arg_i64(&corps, "facture")?;
            let montant = arg_dec(&corps, "montant")?;
            let mode = match arg(&corps, "mode")
                .and_then(|v| v.as_str())
                .unwrap_or("virement")
            {
                "cheque" => db::payment::MODE_CHEQUE,
                "especes" => db::payment::MODE_ESPECES,
                "carte" => db::payment::MODE_CARTE,
                "prelevement" => db::payment::MODE_PRELEVEMENT,
                _ => db::payment::MODE_VIREMENT,
            };
            let nouveau = db::payment::NewPayment {
                fk_facture: facture,
                montant,
                fk_paiement: mode,
                fk_compte: arg(&corps, "compte").and_then(|v| v.as_i64()),
                ..Default::default()
            };
            Ok(json!({ "paiement": db::payment::create(pool, ENTITY, &nouveau)? }))
        }
        "solde_banque" => {
            let comptes = db::banque::comptes(pool, ENTITY)?;
            let mut sortie = Vec::new();
            for c in &comptes {
                sortie.push(json!({
                    "id": c.rowid, "ref": c.ref_, "label": c.label,
                    "solde": db::banque::solde(pool, ENTITY, c.rowid)?.normalize().to_string(),
                }));
            }
            Ok(json!(sortie))
        }
        "solde_tva" => {
            let (collectee, deductible, solde) = db::compta::solde_tva(pool, ENTITY)?;
            Ok(json!({
                "collectee": collectee.normalize().to_string(),
                "deductible": deductible.normalize().to_string(),
                "a_payer": solde.normalize().to_string(),
            }))
        }
        "carnet_commandes" => {
            let lignes = db::carnet::ouvertes(pool, ENTITY)?;
            Ok(json!({
                "pieces": lignes.len(),
                "total_ttc": db::carnet::total(&lignes).normalize().to_string(),
                "livre_non_facture": db::carnet::a_facturer(&lignes).normalize().to_string(),
                "commandes": lignes.iter().map(|l| json!({
                    "id": l.rowid, "ref": l.ref_, "tiers": l.tiers,
                    "date": l.date, "statut": l.statut,
                    "total_ttc": l.total_ttc.normalize().to_string(),
                    "facturee": l.facturee, "a_facturer": l.a_facturer(),
                })).collect::<Vec<_>>(),
            }))
        }
        "marge_chantiers" => Ok(json!(db::carnet::marge_par_chantier(pool, ENTITY)?
            .iter()
            .map(|m| json!({
                "chantier": m.projet, "titre": m.titre, "ref": m.ref_,
                "facture": m.ca.normalize().to_string(),
                "achats": m.achats.normalize().to_string(),
                "marge": m.marge().normalize().to_string(),
                "taux": m.taux().map(|t| t.normalize().to_string()),
            }))
            .collect::<Vec<_>>())),
        "lister_chantiers" => {
            let limite = arg(&corps, "limite").and_then(|v| v.as_u64()).unwrap_or(30) as u32;
            Ok(json!(db::projet::list(pool, ENTITY, limite, 0)?
                .iter()
                .map(|p| json!({
                    "id": p.rowid, "ref": p.ref_, "titre": p.title,
                    "client": p.fk_soc, "debut": p.date_start, "fin": p.date_end,
                    "budget": p.budget.map(|b| b.normalize().to_string()),
                    "ouvert": p.est_ouvert(),
                }))
                .collect::<Vec<_>>()))
        }
        "stock_niveaux" => Ok(json!(db::stock::niveaux(pool, ENTITY)?
            .iter()
            .map(|n| json!({
                "produit": n.ref_produit, "designation": n.label_produit,
                "entrepot": n.entrepot,
                "quantite": n.quantite.normalize().to_string(),
            }))
            .collect::<Vec<_>>())),
        "pdf_devis" => {
            let id = arg_i64(&corps, "id")?;
            let chemin = arg(&corps, "chemin")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    std::env::temp_dir()
                        .join(format!("devis-{id}.pdf"))
                        .display()
                        .to_string()
                });
            let rendu = app::devis_pdf(pool, ENTITY, id)?;
            std::fs::write(&chemin, &rendu.bytes)?;
            Ok(json!({
                "chemin": chemin,
                "octets": rendu.bytes.len(),
                "pages": rendu.pages,
            }))
        }
        "lister_contrats" => Ok(json!(db::contrat::list(pool, ENTITY, 100, 0)?
            .iter()
            .map(|c| json!({
                "id": c.rowid, "ref": c.ref_, "tiers": c.fk_soc,
                "debut": c.date_contrat, "fin": c.fin_validite,
                "statut": c.statut,
                "total_ttc": c.total_ttc.map(|v| v.normalize().to_string()),
            }))
            .collect::<Vec<_>>())),
        "creer_contrat" => {
            let fk_soc = arg_i64(&corps, "tiers")?;
            let mut nouveau = db::contrat::NewContrat {
                fk_soc,
                date_contrat: arg(&corps, "debut")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                fin_validite: arg(&corps, "fin")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                ref_customer: None,
                note_private: None,
                note_public: None,
                fk_user_author: None,
            };
            if nouveau.date_contrat.is_none() {
                nouveau.date_contrat = Some(db::aujourdhui(pool)?);
            }
            Ok(json!({ "id": db::contrat::create(pool, ENTITY, &nouveau)? }))
        }
        "mouvement_stock" => {
            let produit = arg_i64(&corps, "produit")?;
            let entrepot = arg_i64(&corps, "entrepot")?;
            let quantite = arg_dec(&corps, "quantite")?;
            let type_ = arg_i64(&corps, "type")? as i16;
            let libelle = arg(&corps, "libelle").and_then(|v| v.as_str());
            let id =
                db::stock::mouvement(pool, ENTITY, produit, entrepot, quantite, type_, libelle)?;
            Ok(json!({ "id": id }))
        }
        autre => return Err(format!("outil inconnu : {autre}").into()),
    };

    Ok(texte(valeur?))
}
