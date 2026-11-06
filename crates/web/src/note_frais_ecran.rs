//! Ecran des notes de frais.
//!
//! Dolibarr a `expensereport.php`. Le principe est simple et doit le rester :
//! quelqu'un avance de l'argent pour l'entreprise, il le declare, quelqu'un
//! d'autre approuve, et la note part en comptabilite.
//!
//! # Ce que l'ecran montre avant tout
//!
//! Le **total a rembourser**, pas le detail ligne par ligne. Un gerant qui
//! approuve veut voir le montant et la periode ; le detail est dessous. Une
//! liste de lignes sans total oblige a additionner a la main, et c'est comme ca
//! qu'on approuve 2 400 € en croyant en voir 240.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::note_frais::{self, NewLigneFrais, NewNoteFrais};
use maud::html;

use crate::{entite, ui, App, Erreur, Res};

/// La liste.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let notes = note_frais::list(&app.pool, entity, 200, 0)?;
    let types = note_frais::types_frais(&app.pool)?;
    let auteurs = db::user::list(&app.pool, entity)?;

    // Le total en attente : c'est le chiffre qui decide si on ouvre l'ecran.
    let en_attente: rust_decimal::Decimal = notes
        .iter()
        .filter(|n| n.statut == note_frais::STATUT_ATTENTE_VALIDATION)
        .map(|n| n.total_ttc)
        .sum();

    Ok(ui::page(
        "frais",
        "frais",
        "Notes de frais",
        html! {
            h1 { (crate::langue::tr("Notes de frais")) }
            p .sous-titre {
                (crate::langue::tr("Ce que quelqu'un a avancé pour l'entreprise, et qui attend d'être remboursé."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("En attente"),
                    &ui::euros(en_attente),
                    Some(&crate::langue::tr("à approuver ou refuser")),
                    en_attente > rust_decimal::Decimal::ZERO,
                ))
                (ui::carte(
                    &crate::langue::tr("Notes"),
                    &notes.len().to_string(),
                    Some(&crate::langue::tr("toutes périodes")),
                    false,
                ))
            }

            h2 { (crate::langue::tr("Nouvelle note")) }
            @if types.is_empty() || auteurs.is_empty() {
                (ui::vide(
                    "Il manque un type de frais actif ou un utilisateur : la saisie n'est pas possible.",
                    None,
                ))
            } @else {
                form method="post" action="/notes-frais" {
                    label for="fk_user_author" { (crate::langue::tr("Auteur")) } " "
                    select name="fk_user_author" id="fk_user_author" {
                        @for u in &auteurs {
                            option value=(u.rowid) { (u.login.clone()) }
                        }
                    }
                    " "
                    label for="fk_c_type_fees" { (crate::langue::tr("Type")) } " "
                    select name="fk_c_type_fees" id="fk_c_type_fees" {
                        @for (id, libelle) in &types {
                            option value=(id) { (libelle.clone()) }
                        }
                    }
                    " "
                    label for="date" { (crate::langue::tr("Date")) } " "
                    input type="date" name="date" id="date" required;
                    br;
                    label for="date_debut" { (crate::langue::tr("Du")) } " "
                    input type="date" name="date_debut" id="date_debut" required;
                    " "
                    label for="date_fin" { (crate::langue::tr("au")) } " "
                    input type="date" name="date_fin" id="date_fin" required;
                    " "
                    label for="comments" { (crate::langue::tr("Commentaire")) } " "
                    input type="text" name="comments" id="comments" required
                          style="min-width:240px" placeholder=(crate::langue::tr("repas client, péage…"));
                    " "
                    label for="qty" { (crate::langue::tr("Qté")) } " "
                    input type="text" name="qty" id="qty" value="1" style="width:70px";
                    " "
                    label for="subprice" { (crate::langue::tr("P.U. HT")) } " "
                    input type="text" name="subprice" id="subprice" style="width:100px" required;
                    " "
                    label for="tva_tx" { (crate::langue::tr("TVA %")) } " "
                    input type="text" name="tva_tx" id="tva_tx" value="20" style="width:70px";
                    " "
                    button type="submit" { (crate::langue::tr("Créer")) }
                }
                p class="aide petite sous-champ" {
                    (crate::langue::tr("Une note peut ensuite recevoir d'autres lignes. Elle se crée en brouillon : rien n'est soumis tant que vous ne l'avez pas décidé."))
                }
            }

            h2 { (crate::langue::tr("Toutes les notes")) }
            @if notes.is_empty() {
                (ui::vide("Aucune note de frais.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Auteur")) }
                            th { (crate::langue::tr("Période")) }
                            th { (crate::langue::tr("Total TTC")) }
                            th { (crate::langue::tr("État")) }
                        }
                    }
                    tbody {
                        @for n in &notes {
                            tr {
                                td .ref {
                                    a href=(format!("/notes-frais/{}", n.rowid)) { (n.ref_.clone()) }
                                }
                                td { (n.auteur.clone().unwrap_or_else(|| "—".to_string())) }
                                td .ref { (n.date_debut.clone()) " → " (n.date_fin.clone()) }
                                td .num { (ui::euros(n.total_ttc)) }
                                td { (pastille(n.statut)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// La fiche d'une note.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let n = note_frais::get(&app.pool, entity, id)?
        .ok_or_else(|| Erreur(format!("note de frais {id} introuvable")))?;
    let lignes = note_frais::lines(&app.pool, entity, id)?;

    Ok(ui::page(
        "frais",
        "frais",
        &n.ref_,
        html! {
            h1 { (n.ref_.clone()) }
            p .sous-titre {
                (crate::langue::tr("Période")) " " (n.date_debut.clone()) " → " (n.date_fin.clone())
                @if let Some(a) = &n.auteur {
                    " · " (a.clone())
                }
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Total à rembourser"),
                    &ui::euros(n.total_ttc),
                    Some(&crate::langue::tr("TTC, tel qu'approuvé")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Dont TVA"),
                    &ui::euros(n.total_tva),
                    Some(&crate::langue::tr("récupérable selon le justificatif")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("État"),
                    &libelle(n.statut),
                    None,
                    n.statut == note_frais::STATUT_REFUSEE,
                ))
            }

            @if let Some(motif) = &n.detail_refuse {
                h2 { (crate::langue::tr("Motif du refus")) }
                (ui::echec(motif))
            }

            h2 { (crate::langue::tr("Lignes")) }
            table {
                thead {
                    tr {
                        th { (crate::langue::tr("Date")) }
                        th { (crate::langue::tr("Type")) }
                        th { (crate::langue::tr("Commentaire")) }
                        th { (crate::langue::tr("Qté")) }
                        th { (crate::langue::tr("P.U. HT")) }
                        th { (crate::langue::tr("Total TTC")) }
                    }
                }
                tbody {
                    @for l in &lignes {
                        tr {
                            td .ref { (l.date.clone()) }
                            td { (l.type_frais.clone().unwrap_or_else(|| "—".to_string())) }
                            td { (l.comments.clone()) }
                            td .num { (l.qty.normalize().to_string()) }
                            td .num { (ui::euros(l.subprice)) }
                            td .num { (ui::euros(l.total_ttc)) }
                        }
                    }
                }
                tfoot {
                    tr {
                        td colspan="5" { (crate::langue::tr("Total")) }
                        td .num { (ui::euros(n.total_ttc)) }
                    }
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @match n.statut {
                0 => {
                    form method="post" action=(format!("/notes-frais/{}/soumettre", n.rowid)) {
                        button type="submit" { (crate::langue::tr("Soumettre pour validation")) }
                    }
                    p class="aide" {
                        (crate::langue::tr("Une note soumise ne se modifie plus : elle attend une décision."))
                    }
                }
                2 => {
                    form method="post" action=(format!("/notes-frais/{}/approuver", n.rowid)) {
                        button type="submit" { (crate::langue::tr("Approuver")) }
                    }
                    form method="post" action=(format!("/notes-frais/{}/refuser", n.rowid))
                         class="haut" {
                        label for="motif" { (crate::langue::tr("Motif du refus")) } " "
                        input type="text" name="motif" id="motif" required
                              style="min-width:340px" placeholder=(crate::langue::tr("justificatif manquant, montant erroné…"));
                        " "
                        button type="submit" class="petit secondaire" {
                            (crate::langue::tr("Refuser"))
                        }
                    }
                }
                _ => {
                    p class="aide" {
                        (crate::langue::tr("Cette note est close."))
                    }
                }
            }
        },
    ))
}

/// Cree une note depuis un formulaire simple : une ligne par envoi.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();

    let entity = entite::courante();
    let auteur = f
        .get("fk_user_author")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .ok_or_else(|| Erreur("auteur manquant".into()))?;
    let type_frais = lire("fk_c_type_fees")
        .parse::<i64>()
        .map_err(|_| Erreur("type de frais manquant".into()))?;

    let mut new = NewNoteFrais {
        fk_user_author: auteur,
        date_debut: lire("date_debut"),
        date_fin: lire("date_fin"),
        note_public: None,
        note_private: None,
        lines: vec![NewLigneFrais {
            fk_c_type_fees: type_frais,
            date: lire("date"),
            comments: lire("comments"),
            qty: lire("qty")
                .parse()
                .map_err(|_| Erreur("quantité invalide".into()))?,
            subprice: lire("subprice")
                .parse()
                .map_err(|_| Erreur("prix unitaire invalide".into()))?,
            tva_tx: lire("tva_tx")
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
        }],
    };
    new.note_private = Some(lire("note_private")).filter(|s| !s.is_empty());

    let id = note_frais::create(&app.pool, entity, &new)?;
    Ok(Redirect::to(&format!("/notes-frais/{id}")))
}

/// Soumet une note.
pub async fn soumettre(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    note_frais::soumettre(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/notes-frais/{id}")))
}

/// Approuve une note.
pub async fn approuver(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    note_frais::approuver(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/notes-frais/{id}")))
}

/// Refuse une note, avec un motif.
pub async fn refuser(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let motif = f
        .get("motif")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("un refus doit être motivé".into()))?;
    note_frais::refuser(&app.pool, entite::courante(), id, &motif)?;
    Ok(Redirect::to(&format!("/notes-frais/{id}")))
}

/// Le libelle d'un statut.
fn libelle(statut: i16) -> String {
    match statut {
        note_frais::STATUT_BROUILLON => "brouillon",
        note_frais::STATUT_ATTENTE_VALIDATION => "en attente",
        note_frais::STATUT_APPROUVEE => "approuvée",
        note_frais::STATUT_PAYEE => "payée",
        note_frais::STATUT_REFUSEE => "refusée",
        _ => "inconnu",
    }
    .to_string()
}

/// La pastille d'etat.
fn pastille(statut: i16) -> maud::Markup {
    let classe = match statut {
        note_frais::STATUT_APPROUVEE | note_frais::STATUT_PAYEE => "p-payee",
        note_frais::STATUT_ATTENTE_VALIDATION => "p-partielle",
        note_frais::STATUT_REFUSEE => "p-impayee",
        _ => "p-brouillon",
    };
    html! { span class=(format!("pastille {classe}")) { (libelle(statut)) } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_statut_a_un_libelle_lisible() {
        assert_eq!(libelle(0), "brouillon");
        assert_eq!(libelle(2), "en attente");
        assert_eq!(libelle(99), "refusée");
        // Un statut inconnu ne doit pas s'afficher comme un nombre.
        assert_eq!(libelle(42), "inconnu");
    }
}
