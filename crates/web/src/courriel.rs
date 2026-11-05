//! L'envoi d'un courriel : la configuration SMTP, lue une fois, et l'envoi.
//!
//! Le serveur SMTP se configure dans `llx_const` (ecran Parametres > E-mails).
//! La variable `DOLIBARR_SMTP_PASSWORD` l'emporte sur la base : un secret n'a
//! pas a trainer dans une table quand le serveur peut le fournir.
//!
//! Ce module existe pour que l'envoi d'une facture depuis sa fiche et celui
//! d'une campagne passent par **le meme** code. Deux copies d'un reglage SMTP
//! finissent toujours par diverger, et c'est l'utilisateur qui le decouvre.

use std::collections::HashMap;

use db::Pool;

/// La configuration SMTP de l'entite, ou le motif du refus.
pub fn config(pool: &Pool) -> Result<mail::Config, String> {
    let const_ou = |cle: &str| {
        db::company::get_const(pool, cle)
            .ok()
            .flatten()
            .unwrap_or_default()
    };

    let hote = const_ou("DOLIBARR_RS_SMTP_HOST");
    if hote.trim().is_empty() {
        return Err("serveur SMTP non renseigné (Paramètres > Réglage des e-mails)".into());
    }
    let port = const_ou("DOLIBARR_RS_SMTP_PORT")
        .parse::<u16>()
        .unwrap_or(587);

    let mut mot_de_passe = const_ou("DOLIBARR_RS_SMTP_MOTDEPASSE");
    if let Ok(v) = std::env::var("DOLIBARR_SMTP_PASSWORD") {
        if !v.is_empty() {
            mot_de_passe = v;
        }
    }

    let config = mail::Config {
        hote: hote.trim().to_string(),
        port,
        utilisateur: const_ou("DOLIBARR_RS_SMTP_UTILISATEUR").trim().to_string(),
        mot_de_passe,
        expediteur: const_ou("DOLIBARR_RS_EMAIL_EXPEDITEUR").trim().to_string(),
        nom_expediteur: const_ou("MAIN_INFO_SOCIETE_NOM").trim().to_string(),
        securite: match const_ou("DOLIBARR_RS_SMTP_SECURITE").trim() {
            "aucune" => mail::Securite::Aucune,
            "directe" => mail::Securite::Directe,
            "starttls" => mail::Securite::Starttls,
            // Par defaut : 465 direct, tout le reste en STARTTLS.
            _ => mail::Config::securite_pour(port),
        },
        delai: 20,
    };

    if config.expediteur.is_empty() {
        return Err(
            "adresse d'expédition non renseignée (Paramètres > Réglage des e-mails)".into(),
        );
    }
    Ok(config)
}

/// Une adresse qui ressemble a une adresse : une arobase, du texte des deux
/// cotes, et pas d'espace.
///
/// Ce n'est pas une validation RFC : c'est un garde-fou contre la faute de
/// frappe et la case vide. Le serveur SMTP reste seul juge.
pub fn adresse_valide(a: &str) -> bool {
    let a = a.trim();
    if a.is_empty() || a.contains(' ') || a.contains('\n') {
        return false;
    }
    match a.split_once('@') {
        Some((avant, apres)) => {
            !avant.is_empty()
                && apres.contains('.')
                && !apres.starts_with('.')
                && !apres.ends_with('.')
        }
        None => false,
    }
}

/// Envoie une facture par e-mail depuis sa fiche, PDF joint.
///
/// Trois refus, et ils sont tous explicites : adresse invalide, facture sans
/// PDF dans la GED, configuration SMTP absente. Une facture qui part sans sa
/// piece jointe n'est pas une facture envoyee.
pub async fn facture(
    axum::extract::State(app): axum::extract::State<crate::App>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> crate::Res<axum::response::Redirect> {
    use axum::response::Redirect;

    let entity = crate::entite::courante();
    let facture = db::invoice::get(&app.pool, entity, id)?
        .ok_or_else(|| crate::Erreur("Facture introuvable".into()))?;

    let champ = |cle: &str| {
        f.get(cle)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let destinataire = champ("destinataire").unwrap_or_default();
    if !adresse_valide(&destinataire) {
        return Ok(Redirect::to(&format!("/factures/{id}?ok=echec-adresse")));
    }
    let sujet = champ("sujet").unwrap_or_else(|| format!("Votre facture {}", facture.ref_));
    let corps = champ("corps").unwrap_or_else(|| {
        format!(
            "Bonjour,\n\nVeuillez trouver ci-joint la facture {} d'un montant de {} euros.\n\n\
             Bien cordialement,\n",
            facture.ref_,
            facture.total_ttc.unwrap_or_default().round_dp(2)
        )
    });

    // La piece jointe : le PDF de la GED, sans quoi il n'y a rien a envoyer.
    let docs = db::ged::pour_element(&app.pool, entity, "facture", id)?;
    let Some((dossier, nom)) = docs
        .iter()
        .find(|(_, nom)| nom.to_lowercase().ends_with(".pdf"))
        .cloned()
    else {
        return Ok(Redirect::to(&format!("/factures/{id}?ok=echec-pdf")));
    };
    let Ok(octets) = std::fs::read(db::ged::racine().join(&dossier).join(&nom)) else {
        crate::journal_erreurs::enregistrer(
            "erreur",
            &format!("facture {id} : PDF illisible ({dossier}/{nom})"),
        );
        return Ok(Redirect::to(&format!("/factures/{id}?ok=echec-pdf")));
    };

    let config = match config(&app.pool) {
        Ok(c) => c,
        Err(motif) => {
            crate::journal_erreurs::enregistrer("erreur", &format!("envoi facture {id} : {motif}"));
            return Ok(Redirect::to(&format!("/factures/{id}?ok=echec-config")));
        }
    };

    let mut message =
        mail::Message::texte(&config.expediteur, &[destinataire.as_str()], &sujet, &corps);
    message.pieces = vec![(nom.clone(), octets, "application/pdf".to_string())];

    // L'appel reseau est bloquant : hors du fil asynchrone.
    let resultat = tokio::task::spawn_blocking(move || mail::envoyer(&config, &message)).await;

    match resultat {
        Ok(Ok(())) => {
            crate::tracer(
                &app,
                facture.fk_soc,
                "facture",
                id,
                &format!("Facture {} envoyée à {destinataire}", facture.ref_),
            );
            let _ = db::events::enregistrer(
                &app.pool,
                &db::events::Nouvel {
                    type_: "DOCUMENT_SENT",
                    entity,
                    fk_user: None,
                    description: &format!("facture {} envoyée à {destinataire}", facture.ref_),
                    ip: "127.0.0.1",
                    agent: None,
                    fk_object: Some(id),
                    methode: None,
                },
            );
            Ok(Redirect::to(&format!("/factures/{id}?ok=envoyee")))
        }
        autre => {
            let motif = match autre {
                Ok(Err(e)) => e.to_string(),
                Err(e) => format!("tache interrompue : {e}"),
                Ok(Ok(())) => String::new(),
            };
            crate::journal_erreurs::enregistrer(
                "erreur",
                &format!("envoi facture {id} à {destinataire} : {motif}"),
            );
            Ok(Redirect::to(&format!("/factures/{id}?ok=echec-envoi")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_adresse_ordinaire_passe() {
        assert!(adresse_valide("contact@maconnerie-architecture.fr"));
        assert!(adresse_valide("a.b+c@exemple.fr"));
        assert!(adresse_valide("  x@y.fr  "));
    }

    #[test]
    fn les_fautes_de_frappe_sont_refusees() {
        assert!(!adresse_valide(""));
        assert!(!adresse_valide("pas une adresse"));
        assert!(!adresse_valide("sans-arobase.fr"));
        assert!(!adresse_valide("@exemple.fr"));
        assert!(!adresse_valide("moi@"));
        assert!(!adresse_valide("moi@exemple"));
        assert!(!adresse_valide("moi@.fr"));
        assert!(!adresse_valide("moi@exemple."));
        assert!(!adresse_valide("a b@exemple.fr"));
    }
}
