//! Les comptes bancaires et le rapprochement.
//!
//! Rapprocher consiste a dire, pour chaque ligne de releve, a quelle ecriture
//! elle correspond. Tant qu'une ligne n'est pas pointee, le solde theorique et
//! le solde bancaire divergent sans qu'on sache pourquoi.

use crate::*;

pub(crate) async fn banques(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let p = &app.pool;
    let liste = db::banque::comptes(p, entite::courante())?;
    let ecritures = db::banque::ecritures(p, entite::courante(), 40)?;

    let mut soldes = Vec::with_capacity(liste.len());
    for c in &liste {
        soldes.push(db::banque::solde(p, entite::courante(), c.rowid)?);
    }
    let nom_compte = |id: Option<i64>| -> String {
        id.and_then(|i| liste.iter().find(|c| c.rowid == i))
            .map(|c| format!("{} — {}", c.ref_, c.label))
            .unwrap_or_else(|| "—".to_string())
    };

    Ok(ui::page(
        "banques",
        "comptes",
        "Banques",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Banques")) }
            p .sous-titre {
                "Le solde se déduit des écritures : il n'est jamais stocké, comme dans Dolibarr. "
                "Les règlements de factures écrivent déjà leur ligne bancaire."
            }

            h2 {(crate::langue::tr("Comptes")) }
            @if liste.is_empty() {
                (ui::vide("Aucun compte bancaire.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Réf.")) } th {(crate::langue::tr("Libellé")) } th {(crate::langue::tr("Banque")) } th {(crate::langue::tr("IBAN")) } th .num {(crate::langue::tr("Solde")) } th { "État" } } }
                    tbody {
                        @for (i, c) in liste.iter().enumerate() {
                            tr {
                                td .ref { (c.ref_) }
                                td { (c.label) }
                                td { (c.banque.clone().unwrap_or_default()) }
                                td .ref { (c.iban.clone().unwrap_or_default()) }
                                td .num { (ui::euros(soldes[i])) }
                                td {
                                    @if c.clos {
                                        span .pastille.p-brouillon { "clos" }
                                    } @else {
                                        span .pastille.p-payee { "ouvert" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Nouveau compte")) }
            form method="post" action="/banques" {
                table {
                    tbody {
                        tr { th {(crate::langue::tr("Référence")) } td { input type="text" name="ref" required style="width:140px"; } }
                        tr { th {(crate::langue::tr("Libellé")) } td { input type="text" name="label" required style="width:100%"; } }
                        tr { th {(crate::langue::tr("Banque")) } td { input type="text" name="banque" style="width:100%"; } }
                        tr { th {(crate::langue::tr("IBAN")) } td { input type="text" name="iban" style="width:100%"; } }
                        tr { th {(crate::langue::tr("BIC")) } td { input type="text" name="bic" style="width:260px"; } }
                    }
                }
                div .actions { button type="submit" {(crate::langue::tr("Créer le compte")) } }
            }

            h2 {(crate::langue::tr("Dernières écritures")) }
            @if ecritures.is_empty() {
                (ui::vide("Aucune écriture. Encaisser une facture en crée une.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Libellé")) } th {(crate::langue::tr("Compte")) } th .num {(crate::langue::tr("Montant")) } th {(crate::langue::tr("Rapprochement")) } } }
                    tbody {
                        @for e in &ecritures {
                            tr {
                                td .ref { (e.dateo.clone().unwrap_or_default()) }
                                td { (e.label.clone().unwrap_or_default()) }
                                td { (nom_compte(e.fk_account)) }
                                td .num { (ui::euros(e.montant)) }
                                td {
                                    form method="post" action="/banques/rapprocher" {
                                        input type="hidden" name="id" value=(e.rowid);
                                        input type="hidden" name="fait" value=(if e.rapproche { "0" } else { "1" });
                                        button type="submit" class="secondaire petit" {
                                            (if e.rapproche { "Rapproché" } else { "Rapprocher" })
                                        }
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

pub(crate) async fn banque_creer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let champ = |c: &str| {
        f.get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let reference = champ("ref").ok_or_else(|| Erreur("référence du compte obligatoire".into()))?;
    let label = champ("label").ok_or_else(|| Erreur("libellé du compte obligatoire".into()))?;
    db::banque::creer_compte(
        &app.pool,
        entite::courante(),
        &reference,
        &label,
        champ("banque").as_deref(),
        champ("iban").as_deref(),
        champ("bic").as_deref(),
    )?;
    Ok(Redirect::to("/banques?ok=compte"))
}

pub(crate) async fn banque_rapprocher(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let id = f
        .get("id")
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| Erreur("écriture non précisée".into()))?;
    let fait = f.get("fait").map(|v| v == "1").unwrap_or(true);
    db::banque::rapprocher(&app.pool, entite::courante(), id, fait)?;
    Ok(Redirect::to("/banques?ok=rapproche"))
}
