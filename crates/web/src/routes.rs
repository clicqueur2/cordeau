//! La table de routes : quelle adresse appelle quel gestionnaire.
//!
//! Elle vivait dans `main` a la suite de l'initialisation, sur quatre cents
//! lignes. Deux consequences ne se voyaient pas : on ne pouvait pas lire le
//! demarrage sans traverser toutes les adresses, et un conflit de route ne se
//! decouvrait qu'a l'execution. Ici, elle forme un bloc qu'on peut relire d'un
//! coup d'oeil.
//!
//! # L'ordre compte
//!
//! `axum` evalue les routes dans l'ordre de declaration : une route a
//! parametre (`/prets/:id`) doit venir **avant** une route fixe du meme
//! prefixe, sinon la seconde est avalee. C'est une contrainte que rien ne
//! signale a la compilation, d'ou ce rappel.
//!
//! # Les deux couches
//!
//! - `garde` verifie la session, les droits et le mode lecture seule ;
//! - `entetes_securite` pose les en-tetes HTTP.
//!
//! La seconde est declaree en dernier, donc en couche **exterieure** : les
//! en-tetes partent aussi avec les redirections vers la page de connexion.

use axum::routing::{get, post};
use axum::Router;

use crate::*;

/// Construit le routeur complet : pages, API et les deux couches.
pub fn routes(app: App) -> Router {
    Router::new()
        .route(
            "/installation",
            get(pages::installation_page).post(pages::installation_creer),
        )
        .route(
            "/connexion",
            get(auth::connexion_page).post(auth::connexion),
        )
        .route("/deconnexion", post(auth::deconnexion))
        .route("/compte", get(auth::compte))
        .route("/entite", post(auth::changer_entite))
        .route("/", get(accueil))
        .route("/recherche", get(recherche))
        .route("/tiers", get(tiers))
        .route("/tiers/:id", get(tiers_fiche))
        .route("/produits", get(produits))
        .route("/banques/rapprochement", get(rapprochement_ecran::page))
        .route(
            "/banques/rapprochement/pointer",
            post(rapprochement_ecran::pointer),
        )
        .route("/caisse", get(pos_ecran::page))
        .route("/caisse/ajouter", post(pos_ecran::ajouter))
        .route("/caisse/vider", post(pos_ecran::vider))
        .route("/caisse/encaisser", post(pos_ecran::encaisser))
        .route("/caisse/sceller", post(pos_ecran::sceller))
        .route("/caisse/archive", get(pos_ecran::archive))
        .route("/caisse/journal", get(pos_ecran::journal))
        .route("/produits/:id", get(produit_fiche))
        .route(
            "/produits/:id/modifier",
            axum::routing::post(produit_enregistrer),
        )
        .route("/devis", get(devis))
        .route("/devis/actions", post(actions_masse::devis))
        .route("/devis/:id", get(devis_fiche))
        .route("/devis/:id/pdf", get(devis_pdf))
        .route("/devis/:id/facturer", post(devis_facturer))
        .route("/devis/:id/situation", post(devis_situation))
        .route("/devis/:id/commander", post(devis_commander))
        .route("/commandes", get(commandes))
        .route("/commandes/actions", post(actions_masse::commandes))
        .route("/colonnes", post(colonnes::enregistrer))
        .route("/style", get(style_page).post(choisir_style))
        .route("/commandes/carnet", get(carnet))
        .route("/commandes/:id", get(commande_fiche))
        .route("/commandes/:id/valider", post(commande_valider))
        .route("/commandes/:id/facturer", post(commande_facturer))
        .route("/commandes/:id/acompte", post(commande_acompte))
        .route("/commandes/:id/pdf", get(commande_pdf))
        .route("/factures", get(factures))
        .route("/factures/actions", post(actions_masse::factures))
        .route("/devises", get(devises_ecran::page))
        .route("/mrp", get(mrp_ecran::page))
        .route("/parametres/modeles-emails", get(modeles_ecran::page))
        .route(
            "/parametres/modeles-emails/creer",
            post(modeles_ecran::creer),
        )
        .route(
            "/parametres/modeles-emails/basculer",
            post(modeles_ecran::basculer),
        )
        .route("/categories", get(categories_ecran::page))
        .route("/categories/creer", post(categories_ecran::creer))
        .route("/categories/affecter", post(categories_ecran::affecter))
        .route("/categories/retirer", post(categories_ecran::retirer))
        .route("/statistiques", get(stats_ecran::statistiques))
        .route("/echeancier", get(stats_ecran::echeancier))
        .route("/devises/figer", post(devises_ecran::figer))
        .route("/factures/:id", get(factures_fiche))
        .route("/factures/:id/pdf", get(factures_pdf))
        .route("/factures/:id/valider", post(factures_valider))
        .route("/factures/:id/envoyer", post(courriel::facture))
        .route("/factures/:id/recurrence", post(factures_recurrence))
        .route("/factures/:id/reglement", post(factures_reglement))
        .route("/factures/:id/avoir", post(factures_avoir))
        .route(
            "/factures/nouveau",
            get(factures_nouveau).post(factures_creer),
        )
        .route("/relances", get(relances))
        // Les chemins fixes passent avant les chemins a parametre : sinon
        // `/chantiers/gantt` est lu comme `/chantiers/:id`, et « gantt » ne se
        // convertit pas en entier.
        .route("/chantiers", get(chantiers).post(chantier_creer))
        .route("/chantiers/gantt", get(gantt_chantiers))
        .route("/chantiers/marges", get(marges_chantiers))
        .route("/chantiers/:id", get(chantier_fiche))
        .route("/chantiers/:id/tache", post(chantier_tache_creer))
        .route(
            "/chantiers/:id/modifier",
            get(pages::chantier_modifier).post(pages::chantier_enregistrer),
        )
        .route(
            "/achats/fournisseur/:id/modifier",
            get(pages::fournisseur_fiche).post(pages::fournisseur_enregistrer),
        )
        .route(
            "/banques/:id/modifier",
            get(pages::compte_fiche).post(pages::compte_enregistrer),
        )
        .route("/achats", get(achats))
        .route(
            "/achats/facture",
            get(achat_facture_nouvelle).post(achat_facture_creer),
        )
        .route("/achats/facture/:id/valider", post(achat_facture_valider))
        .route("/achats/fournisseur", post(achat_fournisseur_creer))
        .route("/tickets", get(pages::tickets).post(pages::ticket_creer))
        .route("/tickets/actions", post(actions_masse::tickets))
        .route(
            "/tickets/:id/fermer",
            axum::routing::post(pages::ticket_fermer),
        )
        .route("/liste/:code", get(pages::liste_simple_page))
        .route(
            "/liste/:code/nouveau",
            get(pages::liste_saisie_form).post(pages::liste_saisie_creer),
        )
        .route(
            "/liste/:code/fiche/:id",
            get(pages::fiche_saisie).post(pages::fiche_saisie_enregistrer),
        )
        .route(
            "/liste/:code/fiche/:id/supprimer",
            post(pages::fiche_saisie_supprimer),
        )
        .route("/contacts", get(pages::contacts).post(pages::contact_creer))
        .route(
            "/contacts/:id/modifier",
            get(pages::contact_fiche).post(pages::contact_enregistrer),
        )
        .route("/tickets/:id", get(pages::ticket_fiche))
        .route(
            "/tickets/:id/statut",
            axum::routing::post(pages::ticket_statut),
        )
        .route("/stock", get(pages::stock))
        .route(
            "/receptions",
            get(reception_ecran::page).post(reception_ecran::creer),
        )
        .route("/receptions/:id", get(reception_ecran::detail))
        .route("/receptions/:id/valider", post(reception_ecran::valider))
        .route(
            "/receptions/:id/receptionner",
            post(reception_ecran::receptionner),
        )
        .route(
            "/receptions/:id/supprimer",
            post(reception_ecran::supprimer),
        )
        .route("/variantes", get(variantes).post(variante_attribut))
        .route("/variantes/:id/valeur", post(variante_valeur))
        .route(
            "/produits/:id/variantes",
            get(produit_variantes).post(produit_variante_creer),
        )
        .route(
            "/produits/:id/variantes/:combinaison/supprimer",
            post(produit_variante_supprimer),
        )
        .route("/reservations", get(reservations).post(calendrier_creer))
        .route("/reservations/:id", get(reservation_calendrier))
        .route("/reservations/:id/plage", post(plage_creer))
        .route("/reservations/:id/reserver", post(reserver))
        .route("/inscriptions", get(inscriptions).post(inscrire))
        .route("/inscriptions/:id/confirmer", post(inscrire_confirmer))
        .route("/inscriptions/:id/annuler", post(inscrire_annuler))
        .route("/inscriptions/:id/present", post(inscrire_present))
        .route("/salaires", get(salaires).post(salaire_creer))
        .route(
            "/recrutement",
            get(recrutement).post(recrutement_offre_creer),
        )
        .route("/recrutement/:id", get(recrutement_offre))
        .route("/recrutement/:id/publier", post(recrutement_publier))
        .route("/recrutement/:id/clore", post(recrutement_clore))
        .route(
            "/recrutement/:id/candidature",
            post(recrutement_candidature),
        )
        .route(
            "/recrutement/candidature/:id/statut",
            post(recrutement_statut_candidature),
        )
        .route("/salaires/:id/payer", post(salaire_payer))
        .route("/salaires/:id/supprimer", post(salaire_supprimer))
        .route("/ressources", get(ressources).post(ressource_creer))
        .route("/ressources/:id", get(ressource_fiche))
        .route("/ressources/:id/affecter", post(ressource_affecter))
        .route("/prets", get(pret_ecran::page).post(pret_ecran::creer))
        .route("/prets/:id", get(pret_ecran::detail))
        .route("/prets/:id/regler", post(pret_ecran::regler))
        .route(
            "/immobilisations",
            get(immobilisation_ecran::page).post(immobilisation_ecran::creer),
        )
        .route("/immobilisations/:id", get(immobilisation_ecran::detail))
        .route(
            "/immobilisations/:id/ceder",
            post(immobilisation_ecran::ceder),
        )
        .route(
            "/mailings",
            get(mailing_ecran::page).post(mailing_ecran::creer),
        )
        .route("/mailings/:id", get(mailing_ecran::detail))
        .route("/mailings/:id/valider", post(mailing_ecran::valider))
        .route("/mailings/:id/approuver", post(mailing_ecran::approuver))
        .route(
            "/mailings/:id/marquer-envoye",
            post(mailing_ecran::marquer_envoye),
        )
        .route(
            "/connaissances",
            get(connaissances_ecran::page).post(connaissances_ecran::creer),
        )
        .route("/connaissances/:id", get(connaissances_ecran::detail))
        .route(
            "/connaissances/:id/publier",
            post(connaissances_ecran::publier),
        )
        .route(
            "/connaissances/:id/depublier",
            post(connaissances_ecran::depublier),
        )
        .route(
            "/inventaires",
            get(inventaire_ecran::page).post(inventaire_ecran::creer),
        )
        .route("/inventaires/:id", get(inventaire_ecran::detail))
        .route("/inventaires/:id/valider", post(inventaire_ecran::valider))
        .route("/achats-fournisseur", get(achats_fournisseur_ecran::page))
        .route(
            "/achats-fournisseur/demande",
            post(achats_fournisseur_ecran::demande),
        )
        .route(
            "/achats-fournisseur/demande/:id/valider",
            post(achats_fournisseur_ecran::demande_valider),
        )
        .route(
            "/achats-fournisseur/demande/:id/cloturer",
            post(achats_fournisseur_ecran::demande_cloturer),
        )
        .route(
            "/achats-fournisseur/demande/:id/refuser",
            post(achats_fournisseur_ecran::demande_refuser),
        )
        .route(
            "/achats-fournisseur/commande",
            post(achats_fournisseur_ecran::commande),
        )
        .route(
            "/livraisons",
            get(livraison_ecran::page).post(livraison_ecran::creer),
        )
        .route("/livraisons/:id/valider", post(livraison_ecran::valider))
        .route("/livraisons/:id/livrer", post(livraison_ecran::livrer))
        .route(
            "/sites",
            get(site_web_ecran::page).post(site_web_ecran::creer_site),
        )
        .route("/sites/:id", get(site_web_ecran::detail))
        .route("/sites/:id/pages", post(site_web_ecran::creer_page))
        .route("/sites/:id/supprimer", post(site_web_ecran::supprimer_site))
        .route("/pages/:id/publier", post(site_web_ecran::publier))
        .route("/pages/:id/depublier", post(site_web_ecran::depublier))
        .route("/pages/:id/supprimer", post(site_web_ecran::supprimer_page))
        .route(
            "/adherents",
            get(adherent_ecran::page).post(adherent_ecran::creer),
        )
        .route("/adherents/:id/cotiser", post(adherent_ecran::cotiser))
        .route("/adherents/:id/suspendre", post(adherent_ecran::suspendre))
        .route("/adherents/:id/reactiver", post(adherent_ecran::reactiver))
        .route(
            "/partenariats",
            get(partenariat_ecran::page).post(partenariat_ecran::creer),
        )
        .route(
            "/partenariats/:id/soumettre",
            post(partenariat_ecran::soumettre),
        )
        .route(
            "/partenariats/:id/accepter",
            post(partenariat_ecran::accepter),
        )
        .route(
            "/partenariats/:id/refuser",
            post(partenariat_ecran::refuser),
        )
        .route(
            "/partenariats/:id/annuler",
            post(partenariat_ecran::annuler),
        )
        .route("/marges", get(marge_ecran::page).post(marge_ecran::creer))
        .route("/marges/:id", get(marge_ecran::detail))
        .route("/marges/:id/ligne", post(marge_ecran::ajouter_ligne))
        .route("/marges/:id/valider", post(marge_ecran::valider))
        .route(
            "/nomenclatures",
            get(nomenclature_ecran::page).post(nomenclature_ecran::creer),
        )
        .route("/nomenclatures/:id", get(nomenclature_ecran::detail))
        .route(
            "/nomenclatures/:id/valider",
            post(nomenclature_ecran::valider),
        )
        .route("/postes", get(poste_ecran::page).post(poste_ecran::creer))
        .route("/postes/:id", get(poste_ecran::detail))
        .route("/postes/:id/modifier", post(poste_ecran::modifier))
        .route("/postes/:id/valider", post(poste_ecran::valider))
        .route("/postes/:id/desactiver", post(poste_ecran::desactiver))
        .route(
            "/postes/:id/ressource",
            post(poste_ecran::attacher_ressource),
        )
        .route("/postes/:id/groupe", post(poste_ecran::attacher_groupe))
        .route(
            "/presets",
            get(preset_ecran::page).post(preset_ecran::appliquer),
        )
        .route("/dons", get(dons_ecran::page).post(dons_ecran::creer))
        .route("/dons/:id", get(dons_ecran::detail))
        .route("/dons/:id/valider", post(dons_ecran::valider))
        .route("/dons/:id/encaisser", post(dons_ecran::encaisser))
        .route(
            "/transferts",
            get(transfert_ecran::page).post(transfert_ecran::creer),
        )
        .route("/transferts/:id", get(transfert_ecran::detail))
        .route("/transferts/:id/expedier", post(transfert_ecran::expedier))
        .route("/transferts/:id/recevoir", post(transfert_ecran::recevoir))
        .route("/conges", get(conges_ecran::page).post(conges_ecran::creer))
        .route("/conges/:id/valider", post(conges_ecran::valider))
        .route("/conges/:id/approuver", post(conges_ecran::approuver))
        .route("/conges/:id/refuser", post(conges_ecran::refuser))
        .route("/conges/:id/annuler", post(conges_ecran::annuler))
        .route("/interventions", get(interv_ecran::page))
        .route("/interventions/nouveau", post(interv_ecran::creer))
        .route("/interventions/:id", get(interv_ecran::detail))
        .route("/interventions/:id/valider", post(interv_ecran::valider))
        .route("/interventions/:id/cloturer", post(interv_ecran::cloturer))
        .route(
            "/notes-frais",
            get(note_frais_ecran::page).post(note_frais_ecran::creer),
        )
        .route("/notes-frais/:id", get(note_frais_ecran::detail))
        .route(
            "/notes-frais/:id/soumettre",
            post(note_frais_ecran::soumettre),
        )
        .route(
            "/notes-frais/:id/approuver",
            post(note_frais_ecran::approuver),
        )
        .route("/notes-frais/:id/refuser", post(note_frais_ecran::refuser))
        .route("/expeditions", get(expedition_ecran::page))
        .route(
            "/expeditions/depuis-commande",
            post(expedition_ecran::depuis_commande),
        )
        .route("/expeditions/:id", get(expedition_ecran::detail))
        .route("/expeditions/:id/valider", post(expedition_ecran::valider))
        .route(
            "/expeditions/:id/expedier",
            post(expedition_ecran::expedier),
        )
        .route(
            "/expeditions/:id/supprimer",
            post(expedition_ecran::supprimer),
        )
        .route(
            "/stock/mouvement",
            axum::routing::post(pages::stock_mouvement),
        )
        .route(
            "/stock/entrepot",
            axum::routing::post(pages::stock_entrepot),
        )
        .route("/banques", get(banques).post(banque_creer))
        .route("/banques/rapprocher", post(banque_rapprocher))
        .route("/documents", get(documents))
        .route("/agenda", get(agenda))
        .route("/reglements", get(pages::reglements))
        .route("/avoirs", get(pages::avoirs))
        .route("/contrats", get(contrats).post(contrat_creer))
        .route("/contrats/:id", get(contrat_fiche))
        .route("/contrats/:id/valider", post(contrat_valider))
        .route("/contrats/:id/clore", post(contrat_clore))
        .route("/contrats/:id/ligne", post(contrat_ligne))
        .route(
            "/contrats/:id/ligne/:ligne/basculer",
            post(contrat_ligne_basculer),
        )
        .route("/parametres/journal", get(pages::journal))
        .route(
            "/parametres/module/:code",
            get(pages::options_module).post(pages::options_module_enregistrer),
        )
        .route(
            "/parametres/groupes",
            get(pages::groupes).post(pages::groupe_creer),
        )
        .route("/parametres/groupes/:id", get(pages::groupe_fiche))
        .route(
            "/parametres/groupes/:id/membres",
            axum::routing::post(pages::groupe_membres_enregistrer),
        )
        .route(
            "/parametres/groupes/:id/droits",
            axum::routing::post(pages::groupe_droits_enregistrer),
        )
        .route(
            "/parametres/utilisateurs/:id/droits",
            get(pages::utilisateur_droits).post(pages::utilisateur_droits_enregistrer),
        )
        .route("/achats/sepa", get(sepa_ecran::page))
        .route("/achats/sepa/fichier", get(sepa_ecran::fichier))
        .route("/campagnes", get(campagnes::page))
        .route("/campagnes/envoyer", post(campagnes::envoyer))
        .route("/recurrences", get(recur_ecran::page))
        .route("/recurrences/generer", post(recur_ecran::generer))
        .route("/parametres/greffons", get(pages::greffons_page))
        .route("/parametres/traduction", get(traduction_ecran::page))
        .route(
            "/parametres/securite",
            get(securite_ecran::page).post(securite_ecran::enregistrer),
        )
        .route("/parametres/journaux", get(journaux_ecran::page))
        .route(
            "/parametres/notifications",
            get(notifications_ecran::page).post(notifications_ecran::enregistrer),
        )
        .route(
            "/parametres/planification",
            get(planification_ecran::page).post(planification_ecran::programmer),
        )
        .route(
            "/parametres/planification/desactiver",
            post(planification_ecran::desactiver),
        )
        .route(
            "/parametres/modeles-pdf",
            get(pdf_modele_ecran::page).post(pdf_modele_ecran::enregistrer),
        )
        .route(
            "/parametres/constantes",
            get(constantes_ecran::page).post(constantes_ecran::ecrire),
        )
        .route("/parametres/tables", get(pages::tables_page))
        .route("/parametres/tables/:nom", get(pages::table_page))
        .route("/parametres/evenements", get(pages::evenements_page))
        .route("/parametres/extrafields", get(extrafields_ecran::liste))
        .route(
            "/parametres/extrafields/:elementtype",
            get(extrafields_ecran::cible),
        )
        .route(
            "/extrafields/:elementtype/:id",
            get(extrafields_ecran::objet).post(extrafields_ecran::enregistrer),
        )
        .route("/parametres/emails/essai", post(pages::email_essai))
        .route(
            "/parametres/reglage/:cle",
            get(pages::reglages).post(pages::reglages_enregistrer),
        )
        .route("/comptabilite", get(comptabilite))
        .route("/comptabilite/grand-livre", get(grand_livre))
        .route("/comptabilite/lettrage", get(lettrage_ecran::page))
        .route(
            "/comptabilite/lettrage/:compte",
            post(lettrage_ecran::lettrer),
        )
        .route("/comptabilite/livre-tenu", get(livre_tenu::page))
        .route("/comptabilite/fec", get(fec_page).post(fec_generer))
        .route("/comptabilite/analytique", get(analytique_ecran::page))
        .route(
            "/comptabilite/analytique/axe",
            post(analytique_ecran::creer_axe),
        )
        .route(
            "/comptabilite/analytique/compte",
            post(analytique_ecran::creer_compte),
        )
        .route(
            "/comptabilite/analytique/affecter",
            post(analytique_ecran::affecter),
        )
        .route(
            "/comptabilite/cloture",
            get(cloture_ecran::page).post(cloture_ecran::cloturer),
        )
        .route("/comptabilite/journal", get(pages::journal_comptable))
        .route(
            "/commandes-fournisseurs",
            get(pages::commandes_fournisseurs),
        )
        .route("/agenda/nouveau", axum::routing::post(pages::agenda_creer))
        .route("/export/fec.txt", get(export_fec))
        .route("/parametres", get(parametres))
        .route("/parametres/societe", post(parametres_societe))
        .route("/parametres/cgv-modele", post(parametres_cgv_modele))
        .route("/parametres/dictionnaires", get(parametres_dictionnaires))
        .route(
            "/parametres/conditions-reglement",
            post(condition_reglement_creer),
        )
        .route(
            "/parametres/conditions-reglement/desactiver",
            post(condition_reglement_desactiver),
        )
        .route(
            "/parametres/modules",
            get(parametres_modules).post(parametres_module_basculer),
        )
        .route("/parametres/base", get(parametres_base))
        .route("/parametres/utilisateurs", get(utilisateurs))
        .route("/parametres/utilisateurs/nouveau", post(utilisateurs_creer))
        .route(
            "/parametres/utilisateurs/:id/basculer",
            post(utilisateurs_basculer),
        )
        .route("/parametres/import", get(import_page).post(import_tiers))
        .route("/parametres/import/produits", post(import_produits))
        .route("/parametres/import/factures", post(import_factures))
        .route("/export/tiers.csv", get(export_tiers))
        .route("/export/ventes.csv", get(export_ventes))
        .route("/export/achats.csv", get(export_achats))
        .route("/export/tva.csv", get(export_tva))
        .route("/export/grand-livre.csv", get(export_grand_livre))
        .route("/export/produits.csv", get(export_produits))
        .route(
            "/parametres/sauvegarde",
            get(sauvegarde_page).post(sauvegarder_maintenant),
        )
        .route("/tiers/nouveau", get(tiers_nouveau).post(tiers_creer))
        .route(
            "/produits/nouveau",
            get(produits_nouveau).post(produits_creer),
        )
        .route("/devis/nouveau", get(devis_nouveau).post(devis_creer))
        .route(
            "/devis/:id/modifier",
            get(devis_modifier).post(devis_enregistrer),
        )
        .route("/devis/:id/valider", post(devis_valider))
        .route("/devis/:id/signer", post(devis_signer))
        .route("/devis/:id/perdu", post(devis_perdu))
        .route(
            "/tiers/:id/modifier",
            get(tiers_modifier).post(tiers_enregistrer),
        )
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            garde::garde,
        ))
        // Posé en dernier, donc en couche extérieure : les en-têtes partent
        // aussi avec les redirections vers la connexion.
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            garde::entetes_securite,
        ))
        .merge(Router::new().nest("/api", api::routes()))
        // Les memes routes sous les noms de Dolibarr : `/api/index.php/proposals`.
        .merge(Router::new().nest("/api/index.php", api::routes_dolibarr()))
        .with_state(app)
}
