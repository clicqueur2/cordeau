//! Notifications : ce qui merite d'etre dit, et a qui.
//!
//! Le noyau Rust ne passe par **aucun intermediaire** : ni n8n, ni service
//! tiers. Deux canaux, et deux seulement :
//!
//! - le **courriel**, avec la configuration SMTP deja utilisee pour les
//!   factures (`courriel::config`) ;
//! - le **webhook**, une requete POST vers une adresse fournie — l'API Telegram
//!   en est un cas, mais on n'en sait rien ici : on poste du JSON, c'est tout.
//!
//! Ce module construit les alertes. Il ne les envoie pas : l'envoi est dans la
//! tache `notifier`, qui a besoin du reseau. Separer les deux rend les alertes
//! verifiables sans serveur.
//!
//! Aucune alerte n'est inventee : chacune vient d'une requete deja ecrite pour
//! le tableau de bord. Une notification qui ne correspond a rien fatigue et se
//! fait ignorer.

use std::error::Error;

use mysql::Pool;

/// Ce qu'une alerte demande : agir vite, ou agir un jour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravite {
    /// De l'argent qui devait rentrer et ne rentre pas.
    Urgent,
    /// Du travail qui s'echappe, ou du stock qui va arreter un chantier.
    Attention,
}

impl Gravite {
    pub fn code(self) -> &'static str {
        match self {
            Gravite::Urgent => "urgent",
            Gravite::Attention => "attention",
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Gravite::Urgent => "Urgent",
            Gravite::Attention => "À surveiller",
        }
    }
}

/// Une alerte, prete a etre dite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alerte {
    pub gravite: Gravite,
    pub titre: String,
    /// Le detail, en une ligne : ce qui est du, par qui, depuis quand.
    pub detail: String,
    /// L'adresse de l'ecran qui permet d'agir.
    pub lien: String,
}

/// La configuration des notifications, rangee dans `llx_const`.
///
/// Pas de table a nous : ces trois reglages sont des constantes, comme tout le
/// reste de Dolibarr, et ils survivent a une reinstallation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Adresse qui recoit le resume. Vide = pas de courriel.
    pub email: String,
    /// URL du webhook. Vide = pas de webhook.
    pub webhook: String,
    /// Nombre de jours de retard avant d'alerter.
    pub jours_retard: i64,
}

pub const CLE_EMAIL: &str = "CORDEAU_NOTIF_EMAIL";
pub const CLE_WEBHOOK: &str = "CORDEAU_NOTIF_WEBHOOK";
pub const CLE_JOURS: &str = "CORDEAU_NOTIF_JOURS";

impl Config {
    /// Vrai si aucun canal n'est configure : rien ne partirait.
    pub fn aucun_canal(&self) -> bool {
        self.email.trim().is_empty() && self.webhook.trim().is_empty()
    }
}

/// Lit la configuration, avec les defauts.
pub fn config(pool: &Pool, entity: i64) -> Result<Config, Box<dyn Error>> {
    let lire = |cle: &str| crate::constantes::lire(pool, entity, cle).unwrap_or(None);
    Ok(Config {
        email: lire(CLE_EMAIL).unwrap_or_default(),
        webhook: lire(CLE_WEBHOOK).unwrap_or_default(),
        // Cinq jours : une facture peut avoir deux jours de retard pour une
        // raison bancaire sans que cela merite un message.
        jours_retard: lire(CLE_JOURS)
            .and_then(|v| v.trim().parse::<i64>().ok())
            .filter(|v| *v >= 0)
            .unwrap_or(5),
    })
}

/// Ecrit la configuration.
pub fn enregistrer(pool: &Pool, entity: i64, config: &Config) -> Result<(), Box<dyn Error>> {
    if config.jours_retard < 0 {
        return Err("le délai de retard ne peut pas être négatif".into());
    }
    if !config.email.trim().is_empty() && !config.email.contains('@') {
        return Err("l'adresse de notification n'est pas une adresse".into());
    }
    if !config.webhook.trim().is_empty() && !config.webhook.starts_with("http") {
        return Err("le webhook doit être une adresse http(s)".into());
    }
    crate::constantes::ecrire(pool, entity, CLE_EMAIL, config.email.trim())?;
    crate::constantes::ecrire(pool, entity, CLE_WEBHOOK, config.webhook.trim())?;
    crate::constantes::ecrire(pool, entity, CLE_JOURS, &config.jours_retard.to_string())?;
    Ok(())
}

/// Les alertes du moment, de la plus urgente a la moins urgente.
pub fn alertes(pool: &Pool, entity: i64, aujourd_hui: &str) -> Result<Vec<Alerte>, Box<dyn Error>> {
    let jours = config(pool, entity)?.jours_retard;
    let mut out = Vec::new();

    // 1. Les factures en retard, au-dela du delai configure.
    let retards = crate::tableau::en_retard(pool, entity, aujourd_hui, 50)?;
    let retenues: Vec<_> = retards.iter().filter(|r| r.jours >= jours).collect();
    if !retenues.is_empty() {
        let total = crate::tableau::total_retard(
            &retenues.iter().map(|r| (*r).clone()).collect::<Vec<_>>(),
        );
        let plus_vieille = retenues.iter().map(|r| r.jours).max().unwrap_or(0);
        out.push(Alerte {
            gravite: Gravite::Urgent,
            titre: format!("{} facture(s) en retard", retenues.len()),
            detail: format!(
                "{} au total, la plus ancienne de {} jours.",
                euros(total),
                plus_vieille
            ),
            lien: "/factures".to_string(),
        });
    }

    // 2. Les devis sans reponse.
    let devis = crate::tableau::devis_sans_reponse(pool, entity, aujourd_hui, 15, 50)?;
    if !devis.is_empty() {
        let montant: rust_decimal::Decimal = devis.iter().map(|d| d.montant).sum();
        out.push(Alerte {
            gravite: Gravite::Attention,
            titre: format!("{} devis sans réponse", devis.len()),
            detail: format!("{} en attente depuis plus de 15 jours.", euros(montant)),
            lien: "/devis".to_string(),
        });
    }

    // 3. Le stock sous le seuil.
    let stock = crate::tableau::stock_bas(pool, entity)?;
    if !stock.is_empty() {
        out.push(Alerte {
            gravite: Gravite::Attention,
            titre: format!("{} produit(s) sous le seuil", stock.len()),
            detail: "à réapprovisionner avant le prochain chantier.".to_string(),
            lien: "/mrp".to_string(),
        });
    }

    Ok(out)
}

/// Le resume en texte, pour un courriel ou un message.
///
/// Il commence par le nombre : sur un telephone, on lit la premiere ligne et
/// rien d'autre.
pub fn resume(alertes: &[Alerte]) -> String {
    if alertes.is_empty() {
        return "Rien à signaler : aucune facture en retard, aucun devis oublié, aucun stock bas."
            .to_string();
    }
    let mut s = format!("{} point(s) à traiter\n", alertes.len());
    for a in alertes {
        s.push_str(&format!(
            "\n[{}] {}\n  {}\n  https://votre-instance{}\n",
            a.gravite.libelle(),
            a.titre,
            a.detail,
            a.lien
        ));
    }
    s
}

/// Un montant en euros, sans dependance : le texte doit partir meme si la couche
/// d'affichage n'est pas chargee.
fn euros(v: rust_decimal::Decimal) -> String {
    format!("{:.2} €", v.round_dp(2))
}

/// Le corps JSON du webhook.
///
/// On poste du **texte** et le detail structure : Telegram accepte `text`, et un
/// service maison lira `alertes`. Les deux sont dans le meme message, donc un
/// seul envoi sert les deux cas.
pub fn charge_webhook(alertes: &[Alerte]) -> String {
    let items: Vec<serde_json::Value> = alertes
        .iter()
        .map(|a| {
            serde_json::json!({
                "gravite": a.gravite.code(),
                "titre": a.titre,
                "detail": a.detail,
                "lien": a.lien,
            })
        })
        .collect();
    serde_json::json!({
        "text": resume(alertes),
        "alertes": items,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alerte(gravite: Gravite, titre: &str) -> Alerte {
        Alerte {
            gravite,
            titre: titre.to_string(),
            detail: "détail".to_string(),
            lien: "/factures".to_string(),
        }
    }

    #[test]
    fn un_resume_vide_le_dit_au_lieu_de_rendre_du_vide() {
        let s = resume(&[]);
        assert!(s.contains("Rien à signaler"), "{s}");
    }

    #[test]
    fn le_resume_commence_par_le_nombre() {
        let s = resume(&[
            alerte(Gravite::Urgent, "A"),
            alerte(Gravite::Attention, "B"),
        ]);
        assert!(s.starts_with("2 point(s) à traiter"), "{s}");
        assert!(s.contains("[Urgent] A"));
        assert!(s.contains("[À surveiller] B"));
    }

    #[test]
    fn le_webhook_porte_le_texte_et_le_detail() {
        let json = charge_webhook(&[alerte(Gravite::Urgent, "3 factures en retard")]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        // `text` : ce que lit Telegram.
        assert!(v["text"].as_str().unwrap().contains("3 factures en retard"));
        // `alertes` : ce que lira un service maison.
        assert_eq!(v["alertes"][0]["gravite"], "urgent");
        assert_eq!(v["alertes"][0]["lien"], "/factures");
    }

    #[test]
    fn une_config_sans_canal_est_reconnue() {
        assert!(Config::default().aucun_canal());
        let c = Config {
            email: "a@b.fr".into(),
            ..Default::default()
        };
        assert!(!c.aucun_canal());
    }

    #[test]
    fn les_codes_de_gravite_sont_stables() {
        assert_eq!(Gravite::Urgent.code(), "urgent");
        assert_eq!(Gravite::Attention.code(), "attention");
    }

    #[test]
    fn un_montant_est_ecrit_en_euros() {
        use std::str::FromStr;
        assert_eq!(
            euros(rust_decimal::Decimal::from_str("120.5").unwrap()),
            "120.50 €"
        );
    }
}
