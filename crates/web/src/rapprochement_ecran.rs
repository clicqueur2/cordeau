//! Ecran de rapprochement bancaire.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Form;
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// Les encaissements a pointer.
pub async fn page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let entity = entite::courante();
    let lignes = db::rapprochement::a_pointer(&app.pool, entity)?;
    let total = db::rapprochement::total(&lignes);
    let ok = q.get("ok").map(|s| s.as_str());

    Ok(ui::page(
        "banques",
        "rapprochement-bancaire",
        "Rapprochement bancaire",
        html! {
            h1 { (crate::langue::tr("Rapprochement bancaire")) }
            p .sous-titre {
                (crate::langue::tr("Ce qui est passé en banque et qui n'a pas encore été pointé. Indiquez le relevé, puis cochez."))
            }

            @if ok == Some("pointe") { (ui::flash("Ligne pointée.")) }

            div .cartes {
                (ui::carte(
                    "À pointer",
                    &lignes.len().to_string(),
                    Some("lignes de banque"),
                    !lignes.is_empty(),
                ))
                (ui::carte("Montant", &ui::euros(total), Some("encaissements en attente"), false))
            }

            @if lignes.is_empty() {
                (ui::vide(
                    "Tout est pointé : aucune ligne de banque en attente.",
                    Some(("/banques", "Voir les comptes")),
                ))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Date")) }
                            th { (crate::langue::tr("Libellé")) }
                            th { (crate::langue::tr("Facture")) }
                            th { (crate::langue::tr("Tiers")) }
                            th .num { (crate::langue::tr("Montant")) }
                            th { (crate::langue::tr("Relevé")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td .ref { (l.date.clone()) }
                                td { (l.libelle.clone()) }
                                td .ref {
                                    @if let Some(f) = &l.facture {
                                        (f.clone())
                                    } @else {
                                        span class="muet" { "—" }
                                    }
                                }
                                td {
                                    @if let Some(t) = &l.tiers {
                                        (t.clone())
                                    }
                                }
                                td .num { (ui::euros(l.montant)) }
                                td colspan="2" {
                                    form method="post" action="/banques/rapprochement/pointer" style="display:flex;gap:8px" {
                                        input type="hidden" name="ligne" value=(l.ligne);
                                        input type="text" name="releve" placeholder=(crate::langue::tr("N° de relevé")) size="12";
                                        button type="submit" class="petit" { (crate::langue::tr("Pointer")) }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            p.muet.petite.haut {
                (crate::langue::tr("Le paiement et sa ligne de banque naissent ensemble à l'encaissement : pointer ne crée rien, cela marque ce qui existe déjà. Le numéro de relevé est conservé pour retrouver, six mois plus tard, avec quel relevé la ligne a été pointée."))
            }
        },
    ))
}

/// Pointe une ligne.
pub async fn pointer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let ligne = f
        .get("ligne")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("ligne de banque manquante".into()))?;
    let releve = f.get("releve").map(|s| s.as_str()).unwrap_or("");

    db::rapprochement::pointer(&app.pool, entite::courante(), ligne, releve)?;
    Ok(Redirect::to("/banques/rapprochement?ok=pointe"))
}
