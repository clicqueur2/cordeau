//! Client de la norme **AFNOR XP Z12-013**.
//!
//! La réforme française ne dit pas « chaque logiciel parlera à chaque
//! plateforme agréée ». La commission AFNOR a publié une **API REST commune**
//! — XP Z12-013, consultation gratuite chez AFNOR, contrats Swagger joints —
//! que 84 % des plateformes agréées déclarent implémenter (sondage DGFiP du
//! 5 mai 2026). Conséquence : **un client, toutes les plateformes** qui
//! exposent cette API. Pas d'agrégateur, pas d'abonnement, pas de verrou.
//!
//! Ce que ce crate fournit : la mécanique d'échange (authentification,
//! émission, suivi du cycle de vie, annuaire), les types de réponse, et un
//! **transport remplaçable** pour que tout soit testable sans réseau.
//!
//! Ce qu'il ne fournit pas : les **routes exactes**. Elles figurent dans le
//! Swagger de la norme, qui évolue (trois versions entre mai 2025 et février
//! 2026). Elles sont donc **paramétrables**, avec les valeurs d'usage en
//! valeurs par défaut, et le jour où une plateforme s'en écarte on change la
//! configuration au lieu de recompiler.
//!
//! Ce que ce crate ne remplace pas : une **plateforme agréée**. Le noyau est
//! une solution compatible : il parle à la PA, il n'en tient pas lieu.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Erreurs
// ---------------------------------------------------------------------------

/// Ce qui peut empêcher un échange d'aboutir.
#[derive(Debug)]
pub enum Erreur {
    /// Le réseau ou le transport a échoué (DNS, TLS, délai).
    Transport(String),
    /// La plateforme a répondu, mais par une erreur.
    Refus {
        /// Code HTTP renvoyé.
        code: u16,
        /// Corps de la réponse, tel quel.
        corps: String,
    },
    /// Le JSON renvoyé n'a pas la forme attendue.
    Format(String),
    /// La configuration est incomplète (URL, identifiant, secret).
    Configuration(String),
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::Transport(m) => write!(f, "transport : {m}"),
            Erreur::Refus { code, corps } => write!(f, "refus {code} : {corps}"),
            Erreur::Format(m) => write!(f, "format : {m}"),
            Erreur::Configuration(m) => write!(f, "configuration : {m}"),
        }
    }
}

impl std::error::Error for Erreur {}

// ---------------------------------------------------------------------------
// Transport remplaçable
// ---------------------------------------------------------------------------

/// Une requête sortante, indépendante de toute bibliothèque HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requete {
    /// Verbe HTTP.
    pub methode: Methode,
    /// URL complète.
    pub url: String,
    /// En-têtes, dans l'ordre.
    pub entetes: Vec<(String, String)>,
    /// Corps de la requête, vide pour un GET.
    pub corps: String,
}

/// Les verbes dont la norme se sert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Methode {
    /// Lecture.
    Get,
    /// Création.
    Post,
    /// Mise à jour.
    Put,
}

impl Methode {
    /// Le mot du verbe dans la requête HTTP.
    pub fn mot(self) -> &'static str {
        match self {
            Methode::Get => "GET",
            Methode::Post => "POST",
            Methode::Put => "PUT",
        }
    }
}

/// La réponse d'un transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reponse {
    /// Code HTTP.
    pub code: u16,
    /// Corps de la réponse.
    pub corps: String,
}

/// Ce qu'un transport doit savoir faire. Le vrai transport vit dans la couche
/// web ; les tests en fournissent un faux, ce qui rend tout cet échange
/// vérifiable sans serveur.
pub trait Transport {
    /// Exécute la requête.
    fn executer(&self, requete: &Requete) -> Result<Reponse, Erreur>;
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Les routes de l'API. Valeurs d'usage, surchargeables : les plateformes
/// suivent la norme, et quand l'une s'en écarte c'est ici qu'on le dit.
#[derive(Debug, Clone)]
pub struct Routes {
    /// Jeton OAuth2.
    pub jeton: String,
    /// Émission d'une facture (Flux 2).
    pub emission: String,
    /// Statuts de cycle de vie (Flux 6).
    pub statuts: String,
    /// Annuaire des assujettis (Flux 11).
    pub annuaire: String,
}

impl Default for Routes {
    fn default() -> Self {
        Self {
            // OAuth2 « client credentials », chemin d'usage.
            jeton: "/oauth2/token".into(),
            emission: "/api/v1/invoices".into(),
            statuts: "/api/v1/invoices/{id}/status".into(),
            annuaire: "/api/v1/directory/{siren}".into(),
        }
    }
}

/// Ce qu'il faut pour parler à une plateforme agréée.
#[derive(Debug, Clone)]
pub struct Configuration {
    /// Racine de l'API, par exemple `https://api.exemple.fr`.
    pub racine: String,
    /// Identifiant client fourni par la plateforme.
    pub identifiant: String,
    /// Secret client fourni par la plateforme.
    pub secret: String,
    /// Routes.
    pub routes: Routes,
}

impl Configuration {
    /// Vérifie que rien d'indispensable ne manque, et rend la racine sans
    /// barre finale (pour ne pas fabriquer `//api`).
    fn verifier(&self) -> Result<&str, Erreur> {
        for (nom, valeur) in [
            ("racine", &self.racine),
            ("identifiant", &self.identifiant),
            ("secret", &self.secret),
        ] {
            if valeur.trim().is_empty() {
                return Err(Erreur::Configuration(format!("{nom} vide")));
            }
        }
        Ok(self.racine.trim_end_matches('/'))
    }
}

// ---------------------------------------------------------------------------
// Types de la norme
// ---------------------------------------------------------------------------

/// Une facture à déposer, au format de la norme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Depot {
    /// Numéro de la facture, celui du noyau.
    pub numero: String,
    /// SIREN du client, pour l'adressage par l'annuaire.
    pub siren_client: Option<String>,
    /// Le document lui-même : XML CII, UBL, ou Factur-X en base64.
    pub contenu: Vec<u8>,
    /// Le format du document, tel que la norme le nomme.
    pub format: FormatDocument,
}

/// Les formats que la norme et EN 16931 reconnaissent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatDocument {
    /// CII (Cross Industry Invoice), ce que produit `facturx`.
    Cii,
    /// UBL, l'autre syntaxe européenne.
    Ubl,
    /// Factur-X : PDF porteur + XML.
    FacturX,
}

impl FormatDocument {
    /// Le code d'échange attendu côté plateforme.
    pub fn code(self) -> &'static str {
        match self {
            FormatDocument::Cii => "CII",
            FormatDocument::Ubl => "UBL",
            FormatDocument::FacturX => "FACTUR-X",
        }
    }
}

/// L'accusé de dépôt : l'identifiant que la plateforme attribue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accuse {
    /// Identifiant de la facture chez la plateforme.
    #[serde(alias = "invoiceId", alias = "id")]
    pub identifiant: String,
    /// Statut à l'instant du dépôt.
    #[serde(default)]
    pub statut: Option<String>,
}

/// Un statut de cycle de vie (Flux 6).
///
/// Les codes exacts sont dans la norme ; on les transporte **tels quels** au
/// lieu de les traduire en une énumération qui se périmerait à la prochaine
/// version du document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statut {
    /// Code du statut, par exemple `RECU` ou `REJETE`.
    #[serde(alias = "status", alias = "code")]
    pub code: String,
    /// Le motif, quand il y en a un (un rejet en porte toujours).
    #[serde(default, alias = "reason", alias = "motif")]
    pub motif: Option<String>,
    /// Date du statut, au format ISO 8601.
    #[serde(default, alias = "date", alias = "timestamp")]
    pub date: Option<String>,
}

impl Statut {
    /// Un statut qui demande une action : rejet, refus.
    pub fn est_un_echec(&self) -> bool {
        let c = self.code.to_uppercase();
        c.contains("REJET") || c.contains("REFUS") || c.contains("REJECT") || c.contains("ERROR")
    }
}

/// Une entrée de l'annuaire : où envoyer la facture de ce client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntreeAnnuaire {
    /// SIREN interrogé.
    #[serde(alias = "siren")]
    pub siren: String,
    /// La plateforme qui détient ce destinataire.
    #[serde(default, alias = "platform", alias = "pa")]
    pub plateforme: Option<String>,
    /// L'adresse de routage, quand l'annuaire en donne une.
    #[serde(default, alias = "routingCode", alias = "adresse")]
    pub adresse: Option<String>,
}

// ---------------------------------------------------------------------------
// Le client
// ---------------------------------------------------------------------------

/// Un client de plateforme agréée, conforme XP Z12-013.
pub struct Client<T: Transport> {
    configuration: Configuration,
    transport: T,
    jeton: std::cell::RefCell<Option<String>>,
}

impl<T: Transport> Client<T> {
    /// Construit le client. Ne fait aucun appel réseau.
    pub fn nouveau(configuration: Configuration, transport: T) -> Self {
        Self {
            configuration,
            transport,
            jeton: std::cell::RefCell::new(None),
        }
    }

    /// Accès au transport, pour observer ce qui est réellement parti.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Obtient un jeton OAuth2 « client credentials », et le garde pour les
    /// appels suivants. On ne redemande pas un jeton par facture : une
    /// plateforme qui facture à l'appel ne doit pas voir d'appels inutiles.
    pub fn authentifier(&self) -> Result<String, Erreur> {
        if let Some(jeton) = self.jeton.borrow().as_ref() {
            return Ok(jeton.clone());
        }
        let racine = self.configuration.verifier()?;
        let corps = format!(
            "grant_type=client_credentials&client_id={}&client_secret={}",
            encode(&self.configuration.identifiant),
            encode(&self.configuration.secret)
        );
        let requete = Requete {
            methode: Methode::Post,
            url: format!("{racine}{}", self.configuration.routes.jeton),
            entetes: vec![(
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            )],
            corps,
        };
        let reponse = self.transport.executer(&requete)?;
        let corps = lire(reponse)?;

        #[derive(Deserialize)]
        struct Jeton {
            access_token: String,
        }
        let jeton: Jeton = serde_json::from_str(&corps)
            .map_err(|e| Erreur::Format(format!("jeton illisible : {e}")))?;
        *self.jeton.borrow_mut() = Some(jeton.access_token.clone());
        Ok(jeton.access_token)
    }

    /// Dépose une facture (Flux 2).
    pub fn emettre(&self, depot: &Depot) -> Result<Accuse, Erreur> {
        let jeton = self.authentifier()?;
        let racine = self.configuration.verifier()?;
        let corps = serde_json::json!({
            "invoiceNumber": depot.numero,
            "buyerSiren": depot.siren_client,
            "format": depot.format.code(),
            "content": base64(&depot.contenu),
        })
        .to_string();

        let requete = Requete {
            methode: Methode::Post,
            url: format!("{racine}{}", self.configuration.routes.emission),
            entetes: vec![
                ("Authorization".into(), format!("Bearer {jeton}")),
                ("Content-Type".into(), "application/json".into()),
            ],
            corps,
        };
        let reponse = self.transport.executer(&requete)?;
        let corps = lire(reponse)?;
        serde_json::from_str(&corps)
            .map_err(|e| Erreur::Format(format!("accuse illisible : {e} (corps : {corps})")))
    }

    /// Demande l'état d'une facture (Flux 6).
    pub fn statut(&self, identifiant: &str) -> Result<Statut, Erreur> {
        let jeton = self.authentifier()?;
        let racine = self.configuration.verifier()?;
        let chemin = self
            .configuration
            .routes
            .statuts
            .replace("{id}", &encode(identifiant));

        let requete = Requete {
            methode: Methode::Get,
            url: format!("{racine}{chemin}"),
            entetes: vec![("Authorization".into(), format!("Bearer {jeton}"))],
            corps: String::new(),
        };
        let reponse = self.transport.executer(&requete)?;
        let corps = lire(reponse)?;
        serde_json::from_str(&corps)
            .map_err(|e| Erreur::Format(format!("statut illisible : {e} (corps : {corps})")))
    }

    /// Interroge l'annuaire pour savoir où envoyer (Flux 11).
    pub fn annuaire(&self, siren: &str) -> Result<EntreeAnnuaire, Erreur> {
        let jeton = self.authentifier()?;
        let racine = self.configuration.verifier()?;
        let chemin = self
            .configuration
            .routes
            .annuaire
            .replace("{siren}", &encode(siren));

        let requete = Requete {
            methode: Methode::Get,
            url: format!("{racine}{chemin}"),
            entetes: vec![("Authorization".into(), format!("Bearer {jeton}"))],
            corps: String::new(),
        };
        let reponse = self.transport.executer(&requete)?;
        let corps = lire(reponse)?;
        serde_json::from_str(&corps)
            .map_err(|e| Erreur::Format(format!("annuaire illisible : {e} (corps : {corps})")))
    }
}

/// Rend le corps d'une réponse, ou l'erreur qu'elle porte.
fn lire(reponse: Reponse) -> Result<String, Erreur> {
    if (200..300).contains(&reponse.code) {
        Ok(reponse.corps)
    } else {
        Err(Erreur::Refus {
            code: reponse.code,
            corps: reponse.corps,
        })
    }
}

/// Échappement pour une chaîne de requête (formulaire ou segment d'URL).
fn encode(valeur: &str) -> String {
    let mut s = String::with_capacity(valeur.len());
    for octet in valeur.bytes() {
        match octet {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                s.push(octet as char);
            }
            b' ' => s.push_str("%20"),
            _ => s.push_str(&format!("%{octet:02X}")),
        }
    }
    s
}

/// Encodage base64 standard, sans dépendance : le contenu d'une facture est
/// petit, la table est suffisante.
fn base64(donnees: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(donnees.len().div_ceil(3) * 4);
    for morceau in donnees.chunks(3) {
        let b = [
            morceau[0],
            *morceau.get(1).unwrap_or(&0),
            *morceau.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(TABLE[(n >> 18) as usize & 63] as char);
        s.push(TABLE[(n >> 12) as usize & 63] as char);
        s.push(if morceau.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if morceau.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}
