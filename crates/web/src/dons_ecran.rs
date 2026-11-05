//! Ecran des dons : ce qu'on a promis, ce qui est arrive.
//!
//! Dolibarr a `don.php`. La distinction qui compte est celle entre la
//! **promesse** et l'**encaissement** : un don promis n'est pas de l'argent en
//! tresorerie, et confondre les deux fait depenser ce qu'on n'a pas recu.
//! L'ecran affiche donc les deux totaux, jamais un seul.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use db::dons::{self, NewDon};
use maud::html;
use rust_decimal::Decimal;

use crate::{entite, ui, App, Erreur, Res};

/// La liste et le formulaire.
pub async fn page(State(app): State<App>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let dons = dons::list(&app.pool, entity, 200, 0)?;
    let (promis, encaisse) =
        dons::total(&app.pool, entity).unwrap_or((Decimal::ZERO, Decimal::ZERO));

    Ok(ui::page(
        "dons",
        "dons",
        "Dons",
        html! {
            h1 { (crate::langue::tr("Dons")) }
            p .sous-titre {
                (crate::langue::tr("Un don promis n'est pas de l'argent reçu. L'écran ne confond jamais les deux."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Promis"),
                    &ui::euros(promis),
                    Some(&crate::langue::tr("tous les dons enregistrés")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Encaissé"),
                    &ui::euros(encaisse),
                    Some(&crate::langue::tr("argent réellement reçu")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Reste à encaisser"),
                    &ui::euros(promis - encaisse),
                    None,
                    (promis - encaisse) > Decimal::ZERO,
                ))
            }

            h2 { (crate::langue::tr("Nouveau don")) }
            form method="post" action="/dons" {
                label for="lastname" { (crate::langue::tr("Nom")) } " "
                input type="text" name="lastname" id="lastname" style="width:160px";
                " "
                label for="firstname" { (crate::langue::tr("Prénom")) } " "
                input type="text" name="firstname" id="firstname" style="width:140px";
                " "
                label for="societe" { (crate::langue::tr("Société")) } " "
                input type="text" name="societe" id="societe" style="width:200px";
                br;
                label for="email" { (crate::langue::tr("Courriel")) } " "
                input type="email" name="email" id="email" style="width:220px";
                " "
                label for="amount" { (crate::langue::tr("Montant")) } " "
                input type="text" name="amount" id="amount" required style="width:110px";
                " "
                label for="datedon" { (crate::langue::tr("Date")) } " "
                input type="date" name="datedon" id="datedon";
                " "
                button type="submit" { (crate::langue::tr("Enregistrer la promesse")) }
            }
            p style="color:#78736c;font-size:13px;margin:8px 0 0" {
                (crate::langue::tr("Un nom ou une société suffit. Un don anonyme sans aucune trace de qui l'a reçu ne sert à rien."))
            }

            h2 { (crate::langue::tr("Dons")) }
            @if dons.is_empty() {
                (ui::vide("Aucun don enregistré.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Référence")) }
                            th { (crate::langue::tr("Donateur")) }
                            th { (crate::langue::tr("Date")) }
                            th { (crate::langue::tr("Montant")) }
                            th { (crate::langue::tr("État")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for d in &dons {
                            tr {
                                td .ref {
                                    a href=(format!("/dons/{}", d.rowid)) { (d.ref_.clone()) }
                                }
                                td { (d.donateur()) }
                                td .ref {
                                    (d.datedon.clone().unwrap_or_else(|| "—".to_string()))
                                }
                                td .num { (ui::euros(d.montant)) }
                                td { (pastille(d.statut, d.encaisse)) }
                                td {
                                    a href=(format!("/dons/{}", d.rowid)) class="petit secondaire" {
                                        (crate::langue::tr("Ouvrir"))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// La fiche d'un don, avec validation et encaissement.
pub async fn detail(State(app): State<App>, Path(id): Path<i64>) -> Res<maud::Markup> {
    let entity = entite::courante();
    let d =
        dons::get(&app.pool, entity, id)?.ok_or_else(|| Erreur(format!("don {id} introuvable")))?;
    let comptes = db::banque::comptes(&app.pool, entity)?;
    let moyens = db::dict::modes_paiement(&app.pool)?;

    Ok(ui::page(
        "dons",
        "dons",
        &d.ref_,
        html! {
            h1 { (d.donateur()) }
            p .sous-titre {
                (d.ref_.clone()) " · " (ui::euros(d.montant))
                @if let Some(j) = &d.datedon {
                    " · " (j.clone())
                }
            }

            table {
                tbody {
                    @if let Some(e) = &d.email {
                        tr { th { (crate::langue::tr("Courriel")) } td { (e.clone()) } }
                    }
                    @if let Some(s) = &d.societe {
                        tr { th { (crate::langue::tr("Société")) } td { (s.clone()) } }
                    }
                    tr {
                        th { (crate::langue::tr("État")) }
                        td { (pastille(d.statut, d.encaisse)) }
                    }
                }
            }

            h2 { (crate::langue::tr("Actions")) }
            @if d.encaisse {
                p style="color:#78736c;font-size:13.5px" {
                    (crate::langue::tr("Ce don est encaissé. Il ne se supprime plus et ne s'encaisse plus une seconde fois."))
                }
            } @else {
                @if d.statut != dons::STATUT_VALIDE {
                    form method="post" action=(format!("/dons/{}/valider", d.rowid)) {
                        button type="submit" { (crate::langue::tr("Valider")) }
                    }
                    p style="color:#78736c;font-size:13.5px" {
                        (crate::langue::tr("Valider contrôle la promesse. On n'encaisse pas une promesse que personne n'a vérifiée."))
                    }
                } @else {
                    @if comptes.is_empty() || moyens.is_empty() {
                        (ui::vide(
                            "Il faut un compte bancaire et un moyen de paiement pour encaisser.",
                            Some(("/banques", "Voir les comptes")),
                        ))
                    } @else {
                        form method="post" action=(format!("/dons/{}/encaisser", d.rowid)) {
                            label for="fk_bank" { (crate::langue::tr("Compte")) } " "
                            select name="fk_bank" id="fk_bank" {
                                @for c in &comptes {
                                    option value=(c.rowid) { (c.label.clone()) }
                                }
                            }
                            " "
                            label for="fk_typepayment" { (crate::langue::tr("Moyen")) } " "
                            select name="fk_typepayment" id="fk_typepayment" {
                                @for m in &moyens {
                                    option value=(m.id) { (m.libelle.clone()) }
                                }
                            }
                            " "
                            label for="date" { (crate::langue::tr("Date")) } " "
                            input type="date" name="date" id="date" required;
                            " "
                            button type="submit" { (crate::langue::tr("Encaisser")) }
                        }
                        p style="color:#78736c;font-size:13.5px" {
                            (crate::langue::tr("L'encaissement écrit un règlement réel : le montant est recopié du don, jamais ressaisi."))
                        }
                    }
                }
            }
        },
    ))
}

/// Cree une promesse de don.
pub async fn creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let lire = |cle: &str| f.get(cle).map(|s| s.trim().to_string()).unwrap_or_default();
    let option = |cle: &str| {
        let v = lire(cle);
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    };

    let login = crate::utilisateur::courant()
        .ok_or_else(|| Erreur("aucun utilisateur en session".into()))?;
    let entity = entite::courante();
    let auteur = db::user::par_login(&app.pool, &login)?
        .ok_or_else(|| Erreur(format!("utilisateur {login} introuvable")))?
        .rowid;

    let new = NewDon {
        lastname: lire("lastname"),
        firstname: option("firstname"),
        societe: option("societe"),
        email: option("email"),
        address: None,
        zip: None,
        town: None,
        fk_country: 0,
        amount: lire("amount")
            .parse::<Decimal>()
            .map_err(|_| Erreur("montant invalide".into()))?,
        datedon: option("datedon"),
        fk_projet: None,
        note_private: None,
        note_public: None,
        fk_user_author: auteur,
    };
    let id = dons::create(&app.pool, entity, &new)?;
    Ok(Redirect::to(&format!("/dons/{id}")))
}

/// Valide un don.
pub async fn valider(State(app): State<App>, Path(id): Path<i64>) -> Res<Redirect> {
    dons::valider(&app.pool, entite::courante(), id)?;
    Ok(Redirect::to(&format!("/dons/{id}")))
}

/// Encaisse un don.
pub async fn encaisser(
    State(app): State<App>,
    Path(id): Path<i64>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let nombre = |cle: &str| -> Result<i64, Erreur> {
        f.get(cle)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
            .parse::<i64>()
            .map_err(|_| Erreur(format!("{cle} manquant")))
    };
    let date = f
        .get("date")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Erreur("date d'encaissement manquante".into()))?;

    dons::encaisser(
        &app.pool,
        entite::courante(),
        id,
        nombre("fk_bank")?,
        nombre("fk_typepayment")?,
        &date,
    )?;
    Ok(Redirect::to(&format!("/dons/{id}")))
}

/// La pastille : promis, validé, encaissé.
fn pastille(statut: i16, encaisse: bool) -> maud::Markup {
    let (classe, texte) = if encaisse {
        ("p-payee", "encaissé")
    } else if statut == dons::STATUT_VALIDE {
        ("p-partielle", "validé")
    } else {
        ("p-brouillon", "promis")
    };
    html! { span class=(format!("pastille {classe}")) { (crate::langue::tr(texte)) } }
}
