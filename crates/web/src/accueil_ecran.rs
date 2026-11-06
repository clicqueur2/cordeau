//! Accueil et recherche : la premiere page, et la barre de recherche.
//!
//! Le tableau de bord repond a une seule question : « qu'est-ce qui reclame une
//! decision aujourd'hui ». Il ne montre donc que ce qui est en retard, en
//! attente ou sous le seuil — pas des totaux qui ne changent rien.
//!
//! La recherche traverse tiers, produits, devis, factures et commandes : c'est
//! le raccourci de celui qui connait le nom du client et ne veut pas ouvrir
//! quatre menus.
//!
//! Les petites aides (`nom_tiers`, `pluriel`, `message`) vivent ici parce que
//! le tableau de bord est le premier endroit ou elles servent ; elles sont
//! reexportees par la racine pour que les autres ecrans les trouvent.

use crate::*;

pub(crate) fn aujourdhui(pool: &Pool) -> Result<String, Erreur> {
    Ok(db::aujourdhui(pool)?)
}

pub(crate) fn en_retard(echeance: Option<&str>, statut: i16, paye: i16, jour: &str) -> bool {
    statut >= db::invoice::STATUT_VALIDEE
        && paye == 0
        && echeance.map(|d| d < jour).unwrap_or(false)
}

pub fn nom_tiers(pool: &Pool, fk: Option<i64>) -> Res<String> {
    let Some(id) = fk else {
        return Ok(String::new());
    };
    let entity = entite::courante();
    if let Some(p) = db::party::get(pool, entity, id)? {
        return Ok(p.nom);
    }
    // Le tiers existe, mais dans une autre societe : ce n'est pas une donnee
    // manquante, c'est un rattachement a corriger. Le dire evite de chercher
    // un tiers qu'on a sous les yeux dans l'autre entite.
    Ok(match db::party::entity_du_tiers(pool, id)? {
        Some(autre) => format!("(tiers {id} de la societe {autre})"),
        None => format!("(tiers {id} absent)"),
    })
}

pub(crate) fn pluriel(n: usize, singulier: &str, pluriel: &str) -> String {
    if n > 1 {
        format!("{n} {pluriel}")
    } else {
        format!("{n} {singulier}")
    }
}

pub fn message(ok: Option<&String>) -> Markup {
    match ok.map(|s| s.as_str()) {
        Some("reglement") => ui::flash("Règlement enregistré."),
        Some("validee") => ui::flash("Facture validée."),
        // La validation a reussi, mais la piece n'a pas pu etre archivee : le
        // document existe sans son PDF, et c'est exactement ce qu'il faut dire
        // plutot que de laisser croire que tout est pret a envoyer.
        Some("validee-sans-pdf") => ui::echec(
            "Facture validée, mais son PDF n'a pas pu être archivé. Réessayez la génération avant de l'envoyer : le motif est dans le journal des erreurs.",
        ),
        Some("envoyee") => ui::flash("Facture envoyée par e-mail, PDF joint."),
        Some("serie") => ui::flash("Série de factures créée : elle sera émise à l'échéance."),
        Some("echec-adresse") => ui::echec("Adresse e-mail invalide : rien n'a été envoyé."),
        Some("echec-pdf") => {
            ui::echec("Aucun PDF dans la GED pour cette facture : validez-la d'abord.")
        }
        Some("echec-config") => {
            ui::echec("Serveur SMTP non configuré : voir Paramètres > E-mails.")
        }
        Some("echec-envoi") => {
            ui::echec("L'envoi a échoué : le motif est dans le journal des erreurs.")
        }
        Some("facture") => ui::flash("Devis transformé en facture."),
        Some("supprime") => ui::flash("Supprimé."),
        Some("societe") => ui::flash("Paramètres enregistrés."),
        Some("enregistre") => ui::flash("Modifications enregistrées."),
        Some("valide") => ui::flash("Devis validé."),
        Some("signe") => ui::flash("Devis signé : la facturation est ouverte."),
        Some("perdu") => ui::flash("Devis marqué perdu."),
        Some("compte") => ui::flash("Compte mis à jour."),
        Some("avoir") => ui::flash("Avoir créé : la facture d'origine est annulée comptablement."),
        _ => html! {},
    }
}

pub(crate) async fn accueil(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let entity = entite::courante();
    let jour = aujourdhui(p)?;

    let chiffres = db::tableau::chiffres(p, entity)?;
    let devis = db::tableau::devis(p, entity, &format!("{}-01-01", &jour[..4]))?;
    let retard = db::tableau::en_retard(p, entity, &jour, 8)?;
    let sans_reponse = db::tableau::devis_sans_reponse(p, entity, &jour, 15, 6)?;
    let stock = db::tableau::stock_bas(p, entity)?;
    let total_retard = db::tableau::total_retard(&retard);

    let serie = db::stats::par_mois(p, entity, &jour, 12)?;
    let maxi = serie
        .iter()
        .map(|m| m.ventes.max(m.achats))
        .fold(Decimal::ZERO, |a, b| if b > a { b } else { a });
    let clients = db::stats::top_clients(p, entity, &format!("{}-01-01", &jour[..4]), 5)?;
    let maxi_client = clients
        .iter()
        .map(|c| c.montant)
        .fold(Decimal::ZERO, |a, b| if b > a { b } else { a });

    let factures = db::invoice::list(p, entity, 6, 0)?;
    let recentes: Vec<_> = factures
        .iter()
        .filter(|f| f.fk_statut >= db::invoice::STATUT_VALIDEE)
        .collect();

    let alertes = retard.len() + sans_reponse.len() + stock.len();

    Ok(ui::page(
        "accueil",
        "",
        "Tableau de bord",
        html! {
            h1 { (crate::langue::tr("Tableau de bord")) }
            p .sous-titre { (crate::langue::tr("Exercice en cours")) " — " (jour.clone()) }

            div .kpis {
                div .kpi.fort {
                    div .titre { (crate::langue::tr("Facturé")) }
                    div .valeur { (ui::euros(chiffres.facture)) }
                    div .note { (chiffres.nb_factures.to_string()) " " (crate::langue::tr("factures validées")) }
                }
                div .kpi.bon {
                    div .titre { (crate::langue::tr("Encaissé")) }
                    div .valeur { (ui::euros(chiffres.encaisse)) }
                    div .note { (crate::langue::tr("sur les factures validées")) }
                }
                div .kpi {
                    div .titre { (crate::langue::tr("Reste à encaisser")) }
                    div .valeur { (ui::euros(chiffres.reste)) }
                    div .note { (crate::langue::tr("encours client")) }
                }
                div .kpi {
                    div .titre { (crate::langue::tr("Devisé")) }
                    div .valeur { (ui::euros(devis.en_cours)) }
                    div .note { (devis.nb_en_cours.to_string()) " " (crate::langue::tr("devis en attente")) }
                }
                div .kpi {
                    div .titre { (crate::langue::tr("Taux de signature")) }
                    div .valeur { (devis.taux.to_string()) " %" }
                    div .note {
                        (ui::euros(devis.signes)) " / " (ui::euros(devis.emis()))
                    }
                }
                div .kpi.alerte {
                    div .titre { (crate::langue::tr("En retard")) }
                    div .valeur { (ui::euros(total_retard)) }
                    div .note { (retard.len().to_string()) " " (crate::langue::tr("factures à relancer")) }
                }
            }

            @let ca_douze = db::tableau_bord::ca_mensuel(p, entite::courante(), 12).unwrap_or_default();
            @if !ca_douze.is_empty() {
                h2 {(crate::langue::tr("Chiffre d'affaires des douze derniers mois")) }
                div .curve {
                    (ui::barres(&ca_douze))
                    p .aide.petite {
                        (crate::langue::tr("Factures validées, hors avoirs, HT."))
                    }
                }
            }

            @if alertes > 0 {
                p .sous-titre style="margin:18px 0 0" {
                    (alertes.to_string()) " " (crate::langue::tr("point(s) à traiter — voir plus bas."))
                }
            } @else {
                (ui::flash("Rien en attente : aucune facture en retard, aucun devis oublié, aucun stock bas."))
            }

            h2 { (crate::langue::tr("Douze derniers mois")) }
            div .curve {
                table {
                    tbody {
                        @for m in &serie {
                            tr {
                                td .ref.champ-court { (m.mois.clone()) }
                                td style="width:46%" {
                                    div .piste {
                                        div .vente style=(format!(
                                            "width:{}%",
                                            if maxi > Decimal::ZERO {
                                                (m.ventes / maxi * Decimal::from(100)).round_dp(1).normalize()
                                            } else { Decimal::ZERO }
                                        )) {}
                                    }
                                }
                                td .num.champ-court { (ui::euros(m.ventes)) }
                                td style="width:30%" {
                                    div .piste {
                                        div .achat style=(format!(
                                            "width:{}%",
                                            if maxi > Decimal::ZERO {
                                                (m.achats / maxi * Decimal::from(100)).round_dp(1).normalize()
                                            } else { Decimal::ZERO }
                                        )) {}
                                    }
                                }
                                td .num style="width:100px;color:#78736c" { (ui::euros(m.achats)) }
                            }
                        }
                    }
                }
                p style="color:#78736c;font-size:12.5px;margin:6px 0 0" {
                    span style="display:inline-block;width:10px;height:10px;background:#8a5a2b;margin-right:5px" {}
                    (crate::langue::tr("Ventes"))
                    "   "
                    span style="display:inline-block;width:10px;height:10px;background:#c3bdb4;margin-right:5px" {}
                    (crate::langue::tr("Achats"))
                }
            }

            @if !retard.is_empty() {
                h2 { (crate::langue::tr("Factures en retard")) }
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Réf.")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Échéance")) }
                            th .num { (crate::langue::tr("Retard")) }
                            th .num { (crate::langue::tr("Reste dû")) }
                        }
                    }
                    tbody {
                        @for f in &retard {
                            tr {
                                td .ref { a href={ "/factures/" (f.rowid) } { (f.reference.clone()) } }
                                td { (f.tiers.clone()) }
                                td .ref { (f.echeance.clone()) }
                                td .num { span .pastille.p-retard { (f.jours.to_string()) " j" } }
                                td .num { (ui::euros(f.reste)) }
                            }
                        }
                    }
                }
            }

            @if !sans_reponse.is_empty() {
                h2 { (crate::langue::tr("Devis sans réponse")) }
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Réf.")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Envoyé le")) }
                            th .num { (crate::langue::tr("Depuis")) }
                            th .num { (crate::langue::tr("TTC")) }
                        }
                    }
                    tbody {
                        @for d in &sans_reponse {
                            tr {
                                td .ref { a href={ "/devis/" (d.rowid) } { (d.reference.clone()) } }
                                td { (d.tiers.clone()) }
                                td .ref { (d.date.clone()) }
                                td .num { (d.jours.to_string()) " j" }
                                td .num { (ui::euros(d.montant)) }
                            }
                        }
                    }
                }
            }

            @if !stock.is_empty() {
                h2 { (crate::langue::tr("Stock sous le seuil")) }
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Réf.")) }
                            th { (crate::langue::tr("Produit")) }
                            th .num { (crate::langue::tr("Stock")) }
                            th .num { (crate::langue::tr("Seuil")) }
                        }
                    }
                    tbody {
                        @for s in &stock {
                            tr {
                                td .ref { (s.reference.clone()) }
                                td { (s.libelle.clone()) }
                                td .num { (s.stock.normalize().to_string()) }
                                td .num { (s.seuil.normalize().to_string()) }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Meilleurs clients de l'année")) }
            @if clients.is_empty() {
                (ui::vide("Aucune facture validée cette année.", None))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Client")) }
                            th .num { (crate::langue::tr("Factures")) }
                            th .num { (crate::langue::tr("Total TTC")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for c in &clients {
                            tr {
                                td { (c.libelle.clone()) }
                                td .num { (c.nombre.to_string()) }
                                td .num { (ui::euros(c.montant)) }
                                td style="width:26%" {
                                    div .piste {
                                        div .vente style=(format!(
                                            "width:{}%",
                                            if maxi_client > Decimal::ZERO {
                                                (c.montant / maxi_client * Decimal::from(100)).round_dp(1).normalize()
                                            } else { Decimal::ZERO }
                                        )) {}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Dernières factures")) }
            @if recentes.is_empty() {
                (ui::vide("Aucune facture pour l'instant.", Some(("/devis", "Partir d'un devis"))))
            } @else {
                table {
                    thead {
                        tr {
                            th { (crate::langue::tr("Réf.")) }
                            th { (crate::langue::tr("Client")) }
                            th { (crate::langue::tr("Échéance")) }
                            th .num { (crate::langue::tr("TTC")) }
                            th .num { (crate::langue::tr("Reste dû")) }
                            th { (crate::langue::tr("Statut")) }
                        }
                    }
                    tbody {
                        @for f in &recentes {
                            tr {
                                td .ref { a href={ "/factures/" (f.rowid) } { (f.ref_.clone()) } }
                                td { (nom_tiers(p, f.fk_soc)?) }
                                td { (f.date_lim_reglement.clone().unwrap_or_default()) }
                                td .num { (ui::euros(f.total_ttc.unwrap_or_default())) }
                                td .num { (ui::euros(db::payment::balance(p, entity, f.rowid)?.reste)) }
                                td { (ui::pastille(f.fk_statut, f.paye, en_retard(f.date_lim_reglement.as_deref(), f.fk_statut, f.paye, &jour))) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn recherche(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let terme = q
        .get("q")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    let vide = terme.is_empty();

    // Un seul mot suffit a chercher : le nom du tiers pour une facture, la
    // reference pour une piece, le libelle pour un produit. Une recherche qui
    // ne regarde qu'un champ par objet ne trouve que ce qu'on tape deja.
    let tiers: Vec<_> = db::party::list(p, entite::courante(), 500, 0)?
        .into_iter()
        .filter(|t| {
            vide || t.nom.to_lowercase().contains(&terme)
                || t.code_client
                    .as_deref()
                    .map(|c| c.to_lowercase().contains(&terme))
                    .unwrap_or(false)
                || t.email
                    .as_deref()
                    .map(|c| c.to_lowercase().contains(&terme))
                    .unwrap_or(false)
        })
        .collect();

    let produits: Vec<_> = db::product::list(p, entite::courante(), 500, 0)?
        .into_iter()
        .filter(|x| {
            vide || x.ref_.to_lowercase().contains(&terme)
                || x.label.to_lowercase().contains(&terme)
        })
        .collect();

    let devis: Vec<_> = db::proposal::list(p, entite::courante(), 200, 0)?
        .into_iter()
        .filter(|d| vide || d.ref_.to_lowercase().contains(&terme))
        .collect();

    let factures: Vec<_> = db::invoice::list(p, entite::courante(), 200, 0)?
        .into_iter()
        .filter(|x| vide || x.ref_.to_lowercase().contains(&terme))
        .collect();

    let commandes: Vec<_> = db::commande::list(p, entite::courante(), 200, 0)?
        .into_iter()
        .filter(|c| vide || c.ref_.to_lowercase().contains(&terme))
        .collect();

    let projets: Vec<_> = db::projet::list(p, entite::courante(), 200, 0)?
        .into_iter()
        .filter(|x| {
            vide || x.title.to_lowercase().contains(&terme)
                || x.ref_.to_lowercase().contains(&terme)
        })
        .collect();

    // La recherche porte aussi sur le **nom du client** : chercher « Dupont »
    // doit sortir ses factures, pas seulement sa fiche.
    let par_client: Vec<i64> = tiers.iter().map(|t| t.rowid).collect();
    let devis: Vec<_> = if par_client.is_empty() {
        devis
    } else {
        db::proposal::list(p, entite::courante(), 200, 0)?
            .into_iter()
            .filter(|d| {
                (vide || d.ref_.to_lowercase().contains(&terme))
                    || d.fk_soc.map(|s| par_client.contains(&s)).unwrap_or(false)
            })
            .collect()
    };
    let factures: Vec<_> = if par_client.is_empty() {
        factures
    } else {
        db::invoice::list(p, entite::courante(), 200, 0)?
            .into_iter()
            .filter(|x| {
                (vide || x.ref_.to_lowercase().contains(&terme))
                    || x.fk_soc.map(|s| par_client.contains(&s)).unwrap_or(false)
            })
            .collect()
    };

    let total = tiers.len()
        + produits.len()
        + devis.len()
        + factures.len()
        + commandes.len()
        + projets.len();

    Ok(ui::page(
        "",
        "",
        "Recherche",
        html! {
            h1 {(crate::langue::tr("Recherche")) }
            form method="get" action="/recherche" {
                div .actions {
                    input type="search" name="q" value=(terme) placeholder="Nom, référence, e-mail…"
                         .champ-large aria-label="Recherche" autofocus;
                    button type="submit" {(crate::langue::tr("Chercher")) }
                }
            }
            p .sous-titre {
                @if vide {
                    "Tapez un nom, une référence, un e-mail : la recherche porte sur les tiers, "
                    "produits, devis, factures, commandes et chantiers."
                } @else {
                    (total.to_string()) " résultat(s) pour « " (terme) " »"
                }
            }

            @if !vide && total == 0 {
                (ui::vide("Rien ne correspond. Essayez le nom du client plutôt que sa référence, ou l'inverse.", None))
            }

            @if !tiers.is_empty() {
                h2 { "Tiers — " (tiers.len().to_string()) }
                table { tbody { @for t in &tiers {
                    tr {
                        td { a href={ "/tiers/" (t.rowid) } { (t.nom) } }
                        td .ref { (t.code_client.clone().unwrap_or_default()) }
                        td { (t.email.clone().unwrap_or_default()) }
                    }
                } } }
            }

            @if !produits.is_empty() {
                h2 { "Produits et services — " (produits.len().to_string()) }
                table { tbody { @for x in &produits {
                    tr {
                        td .ref { (x.ref_) }
                        td { (x.label) }
                    }
                } } }
            }

            @if !devis.is_empty() {
                h2 { "Devis — " (devis.len().to_string()) }
                table { tbody { @for d in &devis {
                    tr {
                        td .ref { a href={ "/devis/" (d.rowid) } { (d.ref_) } }
                        td { (nom_tiers(p, d.fk_soc)?) }
                        td .num { (ui::euros(d.total_ttc.unwrap_or_default())) }
                    }
                } } }
            }

            @if !commandes.is_empty() {
                h2 { "Commandes — " (commandes.len().to_string()) }
                table { tbody { @for c in &commandes {
                    tr {
                        td .ref { (c.ref_) }
                        td { (nom_tiers(p, Some(c.fk_soc))?) }
                        td .num { (ui::euros(c.total_ttc.unwrap_or_default())) }
                    }
                } } }
            }

            @if !factures.is_empty() {
                h2 { "Factures — " (factures.len().to_string()) }
                table { tbody { @for x in &factures {
                    tr {
                        td .ref { a href={ "/factures/" (x.rowid) } { (x.ref_) } }
                        td { (nom_tiers(p, x.fk_soc)?) }
                        td .num { (ui::euros(x.total_ttc.unwrap_or_default())) }
                    }
                } } }
            }

            @if !projets.is_empty() {
                h2 { "Chantiers — " (projets.len().to_string()) }
                table { tbody { @for x in &projets {
                    tr {
                        td .ref { (x.ref_) }
                        td { a href={ "/chantiers/" (x.rowid) } { (x.title) } }
                    }
                } } }
            }
        },
    ))
}
