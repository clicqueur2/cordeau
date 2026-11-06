//! Comptabilite, exports et sauvegarde.
//!
//! Le FEC est le fichier que l'administration demande en cas de controle : son
//! format est impose et une ligne mal formee le rend inexploitable. Les exports
//! CSV suivent la meme logique — separateur et encodage choisis pour qu'un
//! tableur les ouvre sans les casser.

use crate::*;

pub(crate) async fn comptabilite(State(app): State<App>) -> Res<Markup> {
    let p = &app.pool;
    let ventes = db::compta::ventes_par_mois(p, entite::courante(), 24)?;
    let achats = db::compta::achats_par_mois(p, entite::courante(), 24)?;
    let (collectee, deductible, solde) = db::compta::solde_tva(p, entite::courante())?;
    let ca_ht: Decimal = ventes.iter().map(|m| m.total_ht).sum();
    let achats_ht: Decimal = achats.iter().map(|m| m.total_ht).sum();

    Ok(ui::page(
        "comptabilite",
        "journaux",
        "Comptabilité",
        html! {
            h1 {(crate::langue::tr("Comptabilité")) }
            p .sous-titre {
                "Synthèse bâtie sur les factures validées, pas sur un grand livre : "
                "Dolibarr tient le grand livre dans llx_accounting_bookkeeping, qui demande "
                "une saisie par journal."
            }

            div .cartes {
                (ui::carte("Chiffre d'affaires HT", &ui::euros(ca_ht), Some(&pluriel(ventes.len(), "mois", "mois")), false))
                (ui::carte("Achats HT", &ui::euros(achats_ht), None, false))
                (ui::carte("TVA collectée", &ui::euros(collectee), None, false))
                (ui::carte("TVA déductible", &ui::euros(deductible), None, false))
                (ui::carte(
                    if solde >= Decimal::ZERO { "TVA à payer" } else { "Crédit de TVA" },
                    &ui::euros(solde.abs()),
                    Some("indicatif : à vérifier avant déclaration"),
                    solde > Decimal::ZERO,
                ))
            }

            div .actions {
                a .bouton.secondaire href="/comptabilite/grand-livre" {(crate::langue::tr("Grand livre et balance")) }
                a .bouton.secondaire href="/comptabilite/fec" {(crate::langue::tr("Comptabilisation et FEC")) }
            }

            h2 {(crate::langue::tr("Exports comptables")) }
            div .actions {
                form method="get" action="/export/ventes.csv" {
                    input type="date" name="debut" aria-label="Début";
                    input type="date" name="fin" aria-label="Fin";
                    button type="submit" {(crate::langue::tr("Journal des ventes (CSV)")) }
                }
                form method="get" action="/export/achats.csv" {
                    input type="date" name="debut" aria-label="Début";
                    input type="date" name="fin" aria-label="Fin";
                    button type="submit" class="secondaire" {(crate::langue::tr("Journal des achats (CSV)")) }
                }
                form method="get" action="/export/tva.csv" {
                    input type="date" name="debut" aria-label="Début";
                    input type="date" name="fin" aria-label="Fin";
                    button type="submit" class="secondaire" {(crate::langue::tr("TVA par taux (CSV)")) }
                }
            }

            h2 {(crate::langue::tr("Ventes par mois")) }
            @if ventes.is_empty() {
                (ui::vide("Aucune facture validée.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Mois")) } th .num {(crate::langue::tr("Documents")) } th .num {(crate::langue::tr("HT")) } th .num {(crate::langue::tr("TVA")) } th .num {(crate::langue::tr("TTC")) } } }
                    tbody {
                        @for m in &ventes {
                            tr {
                                td .ref { (m.periode) }
                                td .num { (m.documents) }
                                td .num { (ui::euros(m.total_ht)) }
                                td .num { (ui::euros(m.total_tva)) }
                                td .num { (ui::euros(m.total_ttc)) }
                            }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Achats par mois")) }
            @if achats.is_empty() {
                (ui::vide("Aucune facture fournisseur validée.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Mois")) } th .num {(crate::langue::tr("Documents")) } th .num {(crate::langue::tr("HT")) } th .num {(crate::langue::tr("TVA")) } th .num {(crate::langue::tr("TTC")) } } }
                    tbody {
                        @for m in &achats {
                            tr {
                                td .ref { (m.periode) }
                                td .num { (m.documents) }
                                td .num { (ui::euros(m.total_ht)) }
                                td .num { (ui::euros(m.total_tva)) }
                                td .num { (ui::euros(m.total_ttc)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

/// Journal des ventes au format attendu par un cabinet comptable :
/// point-virgule, virgule decimale, dates jj/mm/aaaa, BOM UTF-8.
pub(crate) async fn grand_livre(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let (debut, fin) = (
        q.get("debut").cloned().unwrap_or_default(),
        q.get("fin").cloned().unwrap_or_default(),
    );
    // La table que Dolibarr lit quand elle est remplie, la reconstitution
    // depuis les documents sinon. Voir `grand_livre::ecritures_effectives`.
    let ecritures = db::grand_livre::ecritures_effectives(
        &app.pool,
        entite::courante(),
        Some(debut.as_str()).filter(|d| !d.is_empty()),
        Some(fin.as_str()).filter(|d| !d.is_empty()),
    )?;
    let source =
        db::grand_livre::source(&app.pool, entite::courante()).unwrap_or("source inconnue");
    let balance = db::grand_livre::balance(
        &app.pool,
        entite::courante(),
        Some(debut.as_str()).filter(|d| !d.is_empty()),
        Some(fin.as_str()).filter(|f| !f.is_empty()),
    )?;
    let total_debit: Decimal = balance.iter().map(|b| b.debit).sum();
    let total_credit: Decimal = balance.iter().map(|b| b.credit).sum();

    Ok(ui::page(
        "comptabilite",
        "journaux",
        "Grand livre",
        html! {
            h1 {(crate::langue::tr("Grand livre")) }
            p .sous-titre {
                "Écritures lues dans llx_accounting_bookkeeping quand la base en porte, "
                "reconstituées depuis les documents sinon. Ce n'est pas une comptabilité tenue : "
                "c'est une proposition à valider par l'expert-comptable."
            }
            p style="color:#78736c;font-size:13px;margin:-14px 0 14px" {
                (crate::langue::tr("Source des écritures : ")) (source)
            }
            form method="get" action="/comptabilite/grand-livre" .actions {
                input type="date" name="debut" value=(debut) aria-label="Début";
                input type="date" name="fin" value=(fin) aria-label="Fin";
                button type="submit" {(crate::langue::tr("Filtrer")) }
                a .bouton.secondaire href={ "/export/grand-livre.csv?debut=" (debut) "&fin=" (fin) } {(crate::langue::tr("Exporter le grand livre (CSV)")) }
            }

            h2 {(crate::langue::tr("Balance")) }
            @if balance.is_empty() {
                (ui::vide("Aucune écriture sur la période.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Compte")) } th {(crate::langue::tr("Libellé")) } th .num {(crate::langue::tr("Débit")) } th .num {(crate::langue::tr("Crédit")) } th .num {(crate::langue::tr("Solde")) } } }
                    tbody {
                        @for b in &balance {
                            tr {
                                td .ref { (b.compte) }
                                td { (b.libelle) }
                                td .num { (ui::euros(b.debit)) }
                                td .num { (ui::euros(b.credit)) }
                                td .num { (ui::euros(b.solde())) }
                            }
                        }
                    }
                    tfoot {
                        tr {
                            td colspan="2" .num {(crate::langue::tr("Totaux")) }
                            td .num { (ui::euros(total_debit)) }
                            td .num { (ui::euros(total_credit)) }
                            td .num { (ui::euros(total_debit - total_credit)) }
                        }
                    }
                }
            }

            h2 { "Écritures" }
            @if ecritures.is_empty() {
                (ui::vide("Aucune écriture.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th {(crate::langue::tr("Journal")) } th {(crate::langue::tr("Compte")) } th {(crate::langue::tr("Libellé")) } th .num {(crate::langue::tr("Débit")) } th .num {(crate::langue::tr("Crédit")) } } }
                    tbody {
                        @for e in &ecritures {
                            tr {
                                td .ref { (app::fmt_date_fr(Some(&e.date))) }
                                td .ref { (e.journal) }
                                td .ref { (e.compte) " " (e.libelle_compte) }
                                td { (e.libelle) }
                                td .num { (ui::euros(e.debit)) }
                                td .num { (ui::euros(e.credit)) }
                            }
                        }
                    }
                }
            }
        },
    ))
}

pub(crate) async fn fec_page(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Markup> {
    let (debut, fin) = (
        q.get("debut").cloned().unwrap_or_default(),
        q.get("fin").cloned().unwrap_or_default(),
    );
    let nombre = db::fec::compter(
        &app.pool,
        entite::courante(),
        Some(debut.as_str()).filter(|d| !d.is_empty()),
        Some(fin.as_str()).filter(|d| !d.is_empty()),
    )?;

    Ok(ui::page(
        "comptabilite",
        "journaux",
        "Comptabilisation et FEC",
        html! {
            (message(q.get("ok")))
            h1 {(crate::langue::tr("Comptabilisation et FEC")) }
            p .sous-titre {
                "Les écritures sont générées depuis les documents valides et écrites dans "
                code { "llx_accounting_bookkeeping" } ". L'export suit le format FEC (art. A47 A-1 du LPF)."
            }

            div .actions {
                form method="post" action="/comptabilite/fec" {
                    input type="date" name="debut" value=(debut) aria-label="Début";
                    input type="date" name="fin" value=(fin) aria-label="Fin";
                    button type="submit" {(crate::langue::tr("Générer les écritures")) }
                }
                @if nombre > 0 {
                    a .bouton.secondaire href={ "/export/fec.txt?debut=" (debut) "&fin=" (fin) } {(crate::langue::tr("Exporter le FEC")) }
                }
            }

            table {
                tbody {
                    tr { th {(crate::langue::tr("Écritures en base")) } td .num { (nombre) } }
                    tr { th {(crate::langue::tr("Journaux")) } td {(crate::langue::tr("VE (ventes) · AC (achats) · BQ (banque)")) } }
                }
            }

            p style="color:#78736c;font-size:13px" {
                "Générer remplace les écritures VE/AC/BQ de la période. Une écriture saisie "
                "dans un autre journal n'est jamais touchée."
            }
        },
    ))
}

pub(crate) async fn fec_generer(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let debut = f
        .get("debut")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let fin = f
        .get("fin")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    db::fec::generer(
        &app.pool,
        entite::courante(),
        debut.as_deref(),
        fin.as_deref(),
    )?;
    Ok(Redirect::to(&format!(
        "/comptabilite/fec?ok=genere&debut={}&fin={}",
        debut.unwrap_or_default(),
        fin.unwrap_or_default()
    )))
}

pub(crate) async fn export_fec(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let debut = q
        .get("debut")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "1970-01-01".to_string());
    let fin = q
        .get("fin")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "2099-12-31".to_string());
    let contenu = db::fec::export(&app.pool, entite::courante(), &debut, &fin)?;
    Ok(csv_response(contenu, "FEC.txt"))
}

pub(crate) async fn export_grand_livre(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let ecritures = db::grand_livre::ecritures(
        &app.pool,
        entite::courante(),
        q.get("debut").map(String::as_str),
        q.get("fin").map(String::as_str),
    )?;
    let mut csv =
        String::from("\u{feff}Date;Journal;Compte;Libelle compte;Libelle;Reference;Debit;Credit\n");
    for e in &ecritures {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{}\n",
            csv_date(&e.date),
            e.journal,
            e.compte,
            csv_champ(&e.libelle_compte),
            csv_champ(&e.libelle),
            csv_champ(&e.reference),
            csv_nombre(e.debit),
            csv_nombre(e.credit),
        ));
    }
    Ok(csv_response(csv, "grand-livre.csv"))
}

pub(crate) async fn export_ventes(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let lignes = db::compta::journal_ventes(
        &app.pool,
        entite::courante(),
        q.get("debut").map(String::as_str),
        q.get("fin").map(String::as_str),
    )?;
    let mut csv = String::from("\u{feff}Date;Reference;Type;Client;Total HT;TVA;Total TTC\n");
    for l in &lignes {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{}\n",
            csv_date(&l.date),
            csv_champ(&l.reference),
            csv_champ(&l.type_document),
            csv_champ(&l.tiers),
            csv_nombre(l.total_ht),
            csv_nombre(l.total_tva),
            csv_nombre(l.total_ttc),
        ));
    }
    Ok(csv_response(csv, "journal-ventes.csv"))
}

pub(crate) async fn export_achats(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let lignes = db::compta::journal_achats(
        &app.pool,
        entite::courante(),
        q.get("debut").map(String::as_str),
        q.get("fin").map(String::as_str),
    )?;
    let mut csv = String::from(
        "\u{feff}Date;Reference;Reference fournisseur;Fournisseur;Total HT;TVA;Total TTC\n",
    );
    for l in &lignes {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{}\n",
            csv_date(&l.date),
            csv_champ(&l.reference),
            csv_champ(&l.complement),
            csv_champ(&l.tiers),
            csv_nombre(l.total_ht),
            csv_nombre(l.total_tva),
            csv_nombre(l.total_ttc),
        ));
    }
    Ok(csv_response(csv, "journal-achats.csv"))
}

pub(crate) async fn export_tva(
    State(app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<Response> {
    let lignes = db::compta::tva_par_taux(
        &app.pool,
        entite::courante(),
        q.get("debut").map(String::as_str),
        q.get("fin").map(String::as_str),
    )?;
    let mut csv = String::from("\u{feff}Mois;Sens;Taux de TVA;Base HT;TVA\n");
    for (mois, sens, taux, ht, tva) in &lignes {
        csv.push_str(&format!(
            "{};{};{};{};{}\n",
            csv_champ(mois),
            csv_champ(sens),
            csv_champ(taux.trim_end_matches('0').trim_end_matches('.')),
            csv_nombre(*ht),
            csv_nombre(*tva),
        ));
    }
    Ok(csv_response(csv, "tva-par-taux.csv"))
}

pub(crate) async fn export_tiers(State(app): State<App>) -> Res<Response> {
    let csv = db::impex::exporter_tiers(&app.pool, entite::courante())?;
    Ok(csv_response(csv, "tiers.csv"))
}

pub(crate) async fn export_produits(State(app): State<App>) -> Res<Response> {
    let csv = db::impex::exporter_produits(&app.pool, entite::courante())?;
    Ok(csv_response(csv, "produits.csv"))
}

/// Nombre en ecriture comptable francaise : `1234,56` (virgule, pas de
/// separateur de milliers : Excel les interprete mal en CSV).
pub(crate) fn csv_nombre(v: rust_decimal::Decimal) -> String {
    format!("{:.2}", v).replace('.', ",")
}

pub(crate) fn csv_date(iso: &str) -> String {
    app::fmt_date_fr(Some(iso))
}

/// Echappe un champ pour un CSV a point-virgule.
pub(crate) fn csv_champ(v: &str) -> String {
    let propre = v.replace(';', ",").replace(['\r', '\n'], " ");
    format!("\"{}\"", propre.replace('"', "\"\""))
}

pub(crate) fn csv_response(contenu: String, nom: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{nom}\""),
            ),
        ],
        contenu,
    )
        .into_response()
}

pub(crate) async fn sauvegarde_page(Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    let racine =
        std::env::var("DOLIBARR_SAUVEGARDES").unwrap_or_else(|_| "sauvegardes".to_string());
    let listes = db::sauvegarde::lister(std::path::Path::new(&racine));

    Ok(ui::page(
        "parametres",
        "sauvegarde",
        "Sauvegarde",
        html! {
            @if let Some(fait) = q.get("fait") {
                p .flash { "Sauvegarde créée : " (fait) }
            }
            @if q.contains_key("echec") {
                p .erreur {(crate::langue::tr("La sauvegarde a échoué. Vérifiez que `mysqldump` est installé (variable DOLIBARR_MYSQLDUMP).")) }
            }
            h1 {(crate::langue::tr("Sauvegarde")) }
            p .sous-titre {(crate::langue::tr("Dump SQL complet + dossier documentaire, dans un dossier horodaté.")) }

            form method="post" action="/parametres/sauvegarde" .actions {
                button type="submit" {(crate::langue::tr("Sauvegarder maintenant")) }
                span .sous-titre { "Dossier : " (racine) }
            }

            h2 {(crate::langue::tr("Sauvegardes existantes")) }
            @if listes.is_empty() {
                (ui::vide("Aucune sauvegarde pour l'instant.", None))
            } @else {
                table {
                    thead { tr { th {(crate::langue::tr("Date")) } th .num {(crate::langue::tr("Fichiers")) } } }
                    tbody {
                        @for (nom, n) in &listes {
                            tr { td .ref { (nom) } td .num { (n) } }
                        }
                    }
                }
            }

            h2 {(crate::langue::tr("Restaurer")) }
            p {(crate::langue::tr("La restauration se fait en ligne de commande, c'est volontaire :")) }
            code {(crate::langue::tr("mysql -h 127.0.0.1 -P 3307 -u root -p dolibarr < sauvegardes/<date>/base.sql")) }
        },
    ))
}

pub(crate) async fn sauvegarder_maintenant(State(app): State<App>) -> Res<Redirect> {
    let racine =
        std::env::var("DOLIBARR_SAUVEGARDES").unwrap_or_else(|_| "sauvegardes".to_string());
    match db::sauvegarde::sauvegarder(&app.pool, std::path::Path::new(&racine)) {
        Ok(s) => {
            let nom = s
                .dossier
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(Redirect::to(&format!("/parametres/sauvegarde?fait={nom}")))
        }
        Err(_) => Ok(Redirect::to("/parametres/sauvegarde?echec=1")),
    }
}

pub(crate) fn pdf_response(bytes: &[u8], nom: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{nom}\""),
            ),
        ],
        bytes.to_vec(),
    )
        .into_response()
}
