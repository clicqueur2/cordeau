//! Carnet de commandes et marge par chantier : les deux mesures qui manquaient
//! au pilotage.
//!
//! Le carnet repond a « combien ai-je vendu et pas encore facture ? », la marge
//! a « ce chantier a-t-il tenu ? ». Les deux lisent les tables de Dolibarr sans
//! rien recalculer d'autre que des sommes.

use crate::*;

pub(crate) async fn carnet(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let lignes = db::carnet::ouvertes(p, entite::courante())?;
    let total = db::carnet::total(&lignes);
    let reste = db::carnet::a_facturer(&lignes);
    let pieces = lignes.len();
    let a_facturer = lignes.iter().filter(|l| l.a_facturer()).count();

    let serie: Vec<(String, f64)> = db::carnet::par_mois(&lignes)
        .into_iter()
        .map(|(mois, montant)| (mois, montant.to_string().parse::<f64>().unwrap_or(0.0)))
        .collect();

    Ok(ui::page(
        "devis",
        "carnet",
        "Carnet de commandes",
        html! {
            h1 {(crate::langue::tr("Carnet de commandes")) }
            p .sous-titre {
                (crate::langue::tr("Commandes validées et livrées, facturées ou non. Le lien vers la facture est lu dans llx_element_element."))
            }

            div .kpis {
                div .kpi.fort {
                    div .titre {(crate::langue::tr("Carnet total")) }
                    div .valeur { (ui::euros(total)) }
                    div .note { (pluriel(pieces, "commande ouverte", "commandes ouvertes")) }
                }
                div .kpi.alerte {
                    div .titre {(crate::langue::tr("Livré, pas facturé")) }
                    div .valeur { (ui::euros(reste)) }
                    div .note { (pluriel(a_facturer, "commande à facturer", "commandes à facturer")) }
                }
            }

            @if serie.len() > 1 {
                h2 {(crate::langue::tr("Carnet par mois de commande")) }
                div .curve {
                    (ui::barres(&serie))
                    p .aide.petite {(crate::langue::tr("TTC des commandes ouvertes, par mois.")) }
                }
            }

            h2 {(crate::langue::tr("Commandes ouvertes")) }
            @if lignes.is_empty() {
                (ui::vide("Aucune commande ouverte. Une commande se crée depuis un devis validé.", Some(("/devis", "Voir les devis"))))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Réf.")) }
                            th {(crate::langue::tr("Client")) }
                            th {(crate::langue::tr("Date")) }
                            th {(crate::langue::tr("État")) }
                            th {(crate::langue::tr("Facturée")) }
                            th .num {(crate::langue::tr("TTC")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for l in &lignes {
                            tr {
                                td .ref { a href={ "/commandes/" (l.rowid) } { (l.ref_.clone()) } }
                                td { (l.tiers.clone().unwrap_or_default()) }
                                td .ref { (app::fmt_date_fr(l.date.as_deref())) }
                                td {
                                    @if l.statut == db::commande::STATUT_LIVREE {
                                        span .pastille.p-payee {(crate::langue::tr("livrée")) }
                                    } @else {
                                        span .pastille.p-validee {(crate::langue::tr("validée")) }
                                    }
                                }
                                td {
                                    @if l.facturee {
                                        span .pastille.p-payee {(crate::langue::tr("oui")) }
                                    } @else {
                                        span .pastille.p-brouillon {(crate::langue::tr("non")) }
                                    }
                                }
                                td .num { (ui::euros(l.total_ttc)) }
                                td {
                                    @if l.a_facturer() {
                                        form method="post" action={ "/commandes/" (l.rowid) "/facturer" } {
                                            button type="submit" class="secondaire petit" {(crate::langue::tr("Facturer")) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    tfoot {
                        tr {
                            td colspan="5" .num {(crate::langue::tr("Total")) }
                            td .num { (ui::euros(total)) }
                            td {}
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn marges_chantiers(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let lignes = db::carnet::marge_par_chantier(p, entite::courante())?;
    let ca_total: Decimal = lignes.iter().map(|l| l.ca).sum();
    let achats_total: Decimal = lignes.iter().map(|l| l.achats).sum();
    let marge_total = ca_total - achats_total;
    let taux = if ca_total > Decimal::ZERO {
        Some(money::round_money(
            marge_total * Decimal::from(100) / ca_total,
        ))
    } else {
        None
    };

    Ok(ui::page(
        "chantiers",
        "marges",
        "Marge par chantier",
        html! {
            h1 {(crate::langue::tr("Marge par chantier")) }
            p .sous-titre {
                (crate::langue::tr("Facturé au client moins facturé par les fournisseurs, sur le même fk_projet. Un chantier sans achat n'a pas de marge : il a un chiffre d'affaires."))
            }

            div .kpis {
                div .kpi.fort {
                    div .titre {(crate::langue::tr("Facturé")) }
                    div .valeur { (ui::euros(ca_total)) }
                }
                div .kpi {
                    div .titre {(crate::langue::tr("Achats")) }
                    div .valeur { (ui::euros(achats_total)) }
                }
                div .kpi.bon {
                    div .titre {(crate::langue::tr("Marge")) }
                    div .valeur { (ui::euros(marge_total)) }
                    @if let Some(t) = taux {
                        div .note { (t.to_string()) " %" }
                    }
                }
            }

            @if lignes.is_empty() {
                (ui::vide("Aucun chantier ne porte à la fois des ventes et des achats.", Some(("/chantiers", "Voir les chantiers"))))
            } @else {
                table {
                    thead {
                        tr {
                            th {(crate::langue::tr("Chantier")) }
                            th {(crate::langue::tr("Réf.")) }
                            th .num {(crate::langue::tr("Facturé")) }
                            th .num {(crate::langue::tr("Achats")) }
                            th .num {(crate::langue::tr("Marge")) }
                            th .num {(crate::langue::tr("Taux")) }
                        }
                    }
                    tbody {
                        @for m in &lignes {
                            tr {
                                td { a href={ "/chantiers/" (m.projet) } { (m.titre.clone()) } }
                                td .ref { (m.ref_.clone().unwrap_or_default()) }
                                td .num { (ui::euros(m.ca)) }
                                td .num { (ui::euros(m.achats)) }
                                td .num {
                                    @if m.marge() < Decimal::ZERO {
                                        span .pastille.p-retard { (ui::euros(m.marge())) }
                                    } @else {
                                        (ui::euros(m.marge()))
                                    }
                                }
                                td .num {
                                    @if let Some(t) = m.taux() {
                                        (t.to_string()) " %"
                                    } @else {
                                        "—"
                                    }
                                }
                            }
                        }
                    }
                    tfoot {
                        tr {
                            td colspan="2" .num {(crate::langue::tr("Total")) }
                            td .num { (ui::euros(ca_total)) }
                            td .num { (ui::euros(achats_total)) }
                            td .num { (ui::euros(marge_total)) }
                            td .num {
                                @if let Some(t) = taux { (t.to_string()) " %" } @else { "—" }
                            }
                        }
                    }
                }
            }
        },
    ))
}
