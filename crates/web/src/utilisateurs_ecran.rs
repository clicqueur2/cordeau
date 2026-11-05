//! Utilisateurs : liste, creation, activation.
//!
//! Un utilisateur desactive garde ses traces : il ne peut simplement plus se
//! connecter. Supprimer un compte qui a signe des pieces rendrait ces pieces
//! orphelines — c'est pour cela que la desactivation existe.

use crate::*;

pub(crate) async fn utilisateurs(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let listes = db::user::list(&app.pool, entite::courante())?;
    Ok(ui::page(
        "parametres",
        "utilisateurs",
        "Utilisateurs",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Utilisateurs")) }
            p .sous-titre {(crate::langue::tr("Comptes de la base Dolibarr : les mots de passe créés ici fonctionnent aussi dans Dolibarr.")) }

            @if listes.is_empty() {
                (ui::vide("Aucun compte.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Identifiant")) }
                            th {(crate::langue::tr("Nom")) }
                            th {(crate::langue::tr("E-mail")) }
                            th {(crate::langue::tr("Rôle")) }
                            th { "État" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for u in &listes {
                            tr {
                                td .ref { (u.login) }
                                td { (u.nom_complet()) }
                                td { (u.email.clone().unwrap_or_default()) }
                                td { (if u.admin { "administrateur" } else { "utilisateur" }) }
                                td {
                                    @if u.actif() {
                                        span .pastille.p-payee { "actif" }
                                    } @else {
                                        span .pastille.p-brouillon {(crate::langue::tr("désactivé")) }
                                    }
                                }
                                td {
                                    @if !u.admin {
                                        a .bouton.petit.secondaire href={ "/parametres/utilisateurs/" (u.rowid) "/droits" } {(crate::langue::tr("Droits")) }
                                    }
                                    form method="post" action={ "/parametres/utilisateurs/" (u.rowid) "/basculer" } {
                                        button type="submit" class="petit secondaire" {
                                            (if u.actif() { "Désactiver" } else { "Activer" })
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau compte")) }
            form method="post" action="/parametres/utilisateurs/nouveau" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Identifiant")) } td { input type="text" name="login" required style="width:220px"; } }
                        tr { th {(crate::langue::tr("Mot de passe")) } td { input type="text" name="mdp" required minlength="8" style="width:220px"; } }
                        tr { th {(crate::langue::tr("Prénom")) } td { input type="text" name="prenom" style="width:100%"; } }
                        tr { th {(crate::langue::tr("Nom")) } td { input type="text" name="nom" style="width:100%"; } }
                        tr { th {(crate::langue::tr("E-mail")) } td { input type="email" name="email" style="width:100%"; } }
                        tr {
                            th {(crate::langue::tr("Rôle")) }
                            td {
                                select name="admin" {
                                    option value="0" {(crate::langue::tr("Utilisateur")) }
                                    option value="1" {(crate::langue::tr("Administrateur")) }
                                }
                            }
                        }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le compte")) } }
            }
        },
    ))
}

pub(crate) async fn utilisateurs_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let nouveau = db::user::NouvelUtilisateur {
        login: f.get("login").map(|s| s.trim()).unwrap_or("").to_string(),
        mot_de_passe: f.get("mdp").cloned().unwrap_or_default(),
        nom: f.get("nom").cloned().unwrap_or_default(),
        prenom: f.get("prenom").cloned().unwrap_or_default(),
        email: f.get("email").filter(|s| !s.trim().is_empty()).cloned(),
        admin: f.get("admin").map(|v| v == "1").unwrap_or(false),
    };
    db::user::creer(&app.pool, entite::courante(), &nouveau)?;
    Ok(Redirect::to("/parametres/utilisateurs?ok=compte"))
}

pub(crate) async fn utilisateurs_basculer(
    State(app): State<App>,
    Path(id): Path<i64>,
) -> Res<Redirect> {
    if let Some(u) = db::user::get(&app.pool, id)? {
        db::user::activer(&app.pool, id, !u.actif())?;
    }
    Ok(Redirect::to("/parametres/utilisateurs?ok=compte"))
}
