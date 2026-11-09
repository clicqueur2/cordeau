//! Import de donnees (tiers, produits, factures).
//!
//! Un import se fait en deux temps : on montre ce qui sera cree, puis on cree.
//! Un import qui ecrit directement sans previsualisation est un import qu'on
//! regrette.

use crate::*;

pub(crate) async fn import_page(Query(q): Query<HashMap<String, String>>) -> Res<Markup> {
    Ok(ui::page(
        "parametres",
        "import",
        "Import / Export",
        html! {
            @if let Some(n) = q.get("crees") {
                p .flash { "Import terminé : " (n) " tiers créés." }
            }
            h1 {(crate::langue::tr("Import / Export")) }
            p .sous-titre {(crate::langue::tr("CSV séparé par « ; » ou par « , », avec une ligne d'entête. Les colonnes sont reconnues par leur nom.")) }

            h2 {(crate::langue::tr("Importer des tiers")) }
            p { "Colonnes reconnues : " code { "nom" } " (obligatoire), " code { "adresse" } ", " code { "cp" } ", " code { "ville" } ", " code { "email" } ", " code { "tel" } ", " code { "siret" } ", " code { "tva" } "." }
            form method="post" action="/parametres/import" {
                textarea name="csv" rows="10" class="zone-code" placeholder="nom;ville;email&#10;SARL Exemple;Charly;contact@exemple.fr"
                { "" }
                div .actions {
                    button type="submit" {(crate::langue::tr("Importer")) }
                }
            }

            h2 {(crate::langue::tr("Importer des produits")) }
            p { "Colonnes reconnues : " code { "ref" } " (obligatoire), " code { "libelle" } ", " code { "prix" } ", " code { "tva" } ", " code { "type" } " (produit ou service), " code { "unite" } "." }
            form method="post" action="/parametres/import/produits" {
                textarea name="csv" rows="6" class="zone-code" placeholder="ref;libelle;prix;tva;type&#10;BOR-030;Bordure granit 30 cm;125.50;20;produit" { "" }
                div .actions { button type="submit" {(crate::langue::tr("Importer")) } }
            }

            h2 {(crate::langue::tr("Importer des factures")) }
            p { "Une ligne = une ligne de facture, regroupée par client et référence. Colonnes : " code { "client" } " (obligatoire), " code { "facture" } ", " code { "date" } ", " code { "echeance" } ", " code { "libelle" } ", " code { "qte" } ", " code { "pu" } ", " code { "tva" } "." }
            p .vide {(crate::langue::tr("Les factures importées sont créées puis validées. Le client est créé s'il n'existe pas. La numérotation Dolibarr est attribuée.")) }
            form method="post" action="/parametres/import/factures" {
                textarea name="csv" rows="6" class="zone-code" placeholder="client;facture;date;libelle;qte;pu;tva&#10;SARL Exemple;F2024-001;2024-03-12;Travaux;1;1200;20" { "" }
                div .actions { button type="submit" {(crate::langue::tr("Importer")) } }
            }

            h2 {(crate::langue::tr("Exporter")) }
            table {
                tbody {
                    tr { td { a .bouton.secondaire href="/export/tiers.csv" {(crate::langue::tr("Tiers (CSV)")) } } td {(crate::langue::tr("code client, nom, adresse, CP, ville, e-mail, téléphone, SIRET, TVA")) } }
                    tr { td { a .bouton.secondaire href="/export/produits.csv" {(crate::langue::tr("Produits (CSV)")) } } td {(crate::langue::tr("référence, libellé, prix HT, taux de TVA, type")) } }
                }
            }
        },
    ))
}

pub(crate) async fn import_tiers(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let csv = f.get("csv").cloned().unwrap_or_default();
    if csv.trim().is_empty() {
        return Err(Erreur("aucun contenu à importer".into()));
    }
    let rapport = db::impex::importer_tiers(&app.pool, entite::courante(), &csv)?;
    Ok(ui::page(
        "parametres",
        "import",
        "Import",
        html! {
            h1 {(crate::langue::tr("Résultat de l'import")) }
            div .cartes {
                (ui::carte("Créés", &rapport.crees.to_string(), None, false))
                (ui::carte("Ignorés", &rapport.ignores.to_string(), Some("déjà présents ou sans nom"), false))
                (ui::carte("Erreurs", &rapport.erreurs.len().to_string(), None, !rapport.erreurs.is_empty()))
            }
            @if !rapport.erreurs.is_empty() {
                h2 {(crate::langue::tr("Erreurs")) }
                table {
                    tbody {
                        @for e in &rapport.erreurs { tr { td { (e) } } }
                    }
                }
            }
            div .actions {
                a .bouton href="/tiers" {(crate::langue::tr("Voir les tiers")) }
                a .bouton.secondaire href="/parametres/import" {(crate::langue::tr("Importer un autre fichier")) }
            }
        },
    ))
}

pub(crate) async fn import_produits(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let csv = f.get("csv").cloned().unwrap_or_default();
    if csv.trim().is_empty() {
        return Err(Erreur("aucun contenu à importer".into()));
    }
    let rapport = db::impex::importer_produits(&app.pool, entite::courante(), &csv)?;
    Ok(ui::page(
        "parametres",
        "import",
        "Import produits",
        html! {
            h1 {(crate::langue::tr("Résultat de l'import produits")) }
            div .cartes {
                (ui::carte("Créés", &rapport.crees.to_string(), None, false))
                (ui::carte("Ignorés", &rapport.ignores.to_string(), Some("déjà présents ou sans référence"), false))
                (ui::carte("Erreurs", &rapport.erreurs.len().to_string(), None, !rapport.erreurs.is_empty()))
            }
            @if !rapport.erreurs.is_empty() {
                h2 {(crate::langue::tr("Erreurs")) }
                table { tbody { @for e in &rapport.erreurs { tr { td { (e) } } } } }
            }
            div .actions {
                a .bouton href="/produits" {(crate::langue::tr("Voir les produits")) }
                a .bouton.secondaire href="/parametres/import" {(crate::langue::tr("Retour")) }
            }
        },
    ))
}

pub(crate) async fn import_factures(
    State(app): State<App>,
    Form(f): Form<HashMap<String, String>>,
) -> Res<Markup> {
    let csv = f.get("csv").cloned().unwrap_or_default();
    if csv.trim().is_empty() {
        return Err(Erreur("aucun contenu à importer".into()));
    }
    let rapport = db::impex::importer_factures(&app.pool, entite::courante(), &csv)?;
    Ok(ui::page(
        "parametres",
        "import",
        "Import factures",
        html! {
            h1 {(crate::langue::tr("Résultat de l'import factures")) }
            div .cartes {
                (ui::carte("Créées", &rapport.crees.to_string(), Some("validées automatiquement"), false))
                (ui::carte("Ignorées", &rapport.ignores.to_string(), None, false))
                (ui::carte("Erreurs", &rapport.erreurs.len().to_string(), None, !rapport.erreurs.is_empty()))
            }
            @if !rapport.erreurs.is_empty() {
                h2 {(crate::langue::tr("Erreurs")) }
                table { tbody { @for e in &rapport.erreurs { tr { td { (e) } } } } }
            }
            div .actions {
                a .bouton href="/factures" {(crate::langue::tr("Voir les factures")) }
                a .bouton.secondaire href="/parametres/import" {(crate::langue::tr("Retour")) }
            }
        },
    ))
}
