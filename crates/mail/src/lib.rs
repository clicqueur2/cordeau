//! Client SMTP minimal, pour que les e-mails du noyau partent vraiment.
//!
//! Dolibarr envoie par PHPMailer ; ce noyau envoie lui-meme, sans dependance a
//! PHP. Le dialogue implemente est celui que tout serveur comprend :
//!
//! ```text
//! S: 220 …
//! C: EHLO hote
//! S: 250-…
//! C: STARTTLS            (port 587)
//! S: 220 …
//! C: EHLO hote           (de nouveau, en clair)
//! C: AUTH PLAIN <base64>
//! S: 235 …
//! C: MAIL FROM:<…>
//! C: RCPT TO:<…>
//! C: DATA
//! C: <en-tetes + corps>
//! C: .
//! S: 250 …
//! C: QUIT
//! ```
//!
//! Trois modes : `Directe` (TLS des le depart, port 465), `Starttls` (port 587,
//! le cas d'o2switch et de la plupart des hebergeurs) et `Aucune` (serveur local,
//! utilise uniquement par les tests).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use base64::Engine;

/// Comment le canal est chiffre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Securite {
    /// TLS des la connexion (port 465, « SMTPS »).
    Directe,
    /// Connexion en clair puis STARTTLS (port 587).
    Starttls,
    /// Aucun chiffrement : serveur local de test uniquement.
    Aucune,
}

/// Parametres de connexion.
#[derive(Clone, Debug)]
pub struct Config {
    pub hote: String,
    pub port: u16,
    pub utilisateur: String,
    pub mot_de_passe: String,
    pub expediteur: String,
    /// Nom affiche a cote de l'adresse, facultatif.
    pub nom_expediteur: String,
    pub securite: Securite,
    /// Delai de lecture, en secondes.
    pub delai: u64,
}

impl Config {
    /// Le mode se deduit du port quand on ne le precise pas : 465 direct, 25
    /// ou 587 en STARTTLS.
    pub fn securite_pour(port: u16) -> Securite {
        match port {
            465 => Securite::Directe,
            _ => Securite::Starttls,
        }
    }
}

/// Un message a envoyer.
#[derive(Clone, Debug)]
pub struct Message {
    pub de: String,
    pub a: Vec<String>,
    pub cc: Vec<String>,
    pub sujet: String,
    /// Corps en texte brut. Il est envoye en `multipart/alternative` avec la
    /// version HTML quand elle existe.
    pub texte: String,
    pub html: Option<String>,
    /// Pieces jointes : (nom de fichier, contenu, type MIME).
    pub pieces: Vec<(String, Vec<u8>, String)>,
}

impl Message {
    /// Un message texte simple.
    pub fn texte(de: &str, a: &[&str], sujet: &str, corps: &str) -> Self {
        Self {
            de: de.to_string(),
            a: a.iter().map(|s| s.to_string()).collect(),
            cc: Vec::new(),
            sujet: sujet.to_string(),
            texte: corps.to_string(),
            html: None,
            pieces: Vec::new(),
        }
    }
}

/// Ce qui a echoue, avec le code SMTP quand il y en a un.
#[derive(Debug)]
pub enum Erreur {
    Reseau(String),
    Tls(String),
    /// Le serveur a refuse : code, verbe, et sa reponse.
    Serveur(u16, String, String),
    /// Reponse illisible : le dialogue ne correspond pas au protocole.
    Protocole(String),
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::Reseau(e) => write!(f, "reseau : {e}"),
            Erreur::Tls(e) => write!(f, "TLS : {e}"),
            Erreur::Serveur(c, verbe, rep) => {
                write!(f, "le serveur a refuse {verbe} ({c}) : {rep}")
            }
            Erreur::Protocole(e) => write!(f, "reponse inattendue : {e}"),
        }
    }
}

impl std::error::Error for Erreur {}

/// Envoie un message. Bloquant : l'appelant l'execute hors du fil asynchrone.
pub fn envoyer(config: &Config, message: &Message) -> Result<(), Erreur> {
    if message.a.is_empty() {
        return Err(Erreur::Protocole("aucun destinataire".into()));
    }

    let delai = Duration::from_secs(config.delai.max(5));
    let adresse = format!("{}:{}", config.hote, config.port);
    let brut = TcpStream::connect(&adresse)
        .map_err(|e| Erreur::Reseau(format!("connexion a {adresse} : {e}")))?;
    brut.set_read_timeout(Some(delai)).ok();
    brut.set_write_timeout(Some(delai)).ok();

    match config.securite {
        Securite::Directe => {
            let flux = natif(brut, &config.hote)?;
            dialogue(flux, config, message, false)
        }
        Securite::Aucune => dialogue(Box::new(brut), config, message, false),
        Securite::Starttls => {
            // Premier dialogue en clair, juste pour negocier TLS.
            let mut flux: Canal = Box::new(brut);
            salutation(&mut flux, &config.hote)?;
            commande(&mut flux, "STARTTLS", &["220"])?;
            let flux = natif(flux, &config.hote)?;
            dialogue(flux, config, message, true)
        }
    }
}

type Canal = Box<dyn ReadWrite>;

/// Un flux qui se lit et s'ecrit : socket nue ou socket chiffree.
pub trait ReadWrite: Read + Write + std::fmt::Debug {}
impl<T: Read + Write + std::fmt::Debug> ReadWrite for T {}

fn natif<S: Read + Write + std::fmt::Debug + 'static>(
    flux: S,
    hote: &str,
) -> Result<Canal, Erreur> {
    let connecteur = native_tls::TlsConnector::new()
        .map_err(|e| Erreur::Tls(format!("initialisation : {e}")))?;
    let chiffre = connecteur
        .connect(hote, flux)
        .map_err(|e| Erreur::Tls(format!("negociation avec {hote} : {e}")))?;
    Ok(Box::new(chiffre))
}

/// Lit la reponse du serveur : une ou plusieurs lignes, code a trois chiffres.
fn reponse(flux: &mut Canal) -> Result<(u16, String), Erreur> {
    let mut lecteur = BufReader::new(flux);
    let mut texte = String::new();
    loop {
        let mut ligne = String::new();
        let n = lecteur
            .read_line(&mut ligne)
            .map_err(|e| Erreur::Reseau(format!("lecture : {e}")))?;
        if n == 0 {
            return Err(Erreur::Reseau("le serveur a ferme la connexion".into()));
        }
        let ligne = ligne.trim_end_matches(['\r', '\n']);
        if ligne.len() < 3 {
            return Err(Erreur::Protocole(format!("ligne trop courte : {ligne:?}")));
        }
        let code: u16 = ligne[..3]
            .parse()
            .map_err(|_| Erreur::Protocole(format!("code illisible : {ligne:?}")))?;
        // « 250-… » annonce une suite, « 250 … » termine.
        let suite = ligne.as_bytes().get(3) == Some(&b'-');
        texte.push_str(ligne.get(4..).unwrap_or("").trim());
        texte.push('\n');
        if !suite {
            return Ok((code, texte.trim_end().to_string()));
        }
    }
}

fn ecrire(flux: &mut Canal, ligne: &str) -> Result<(), Erreur> {
    flux.write_all(ligne.as_bytes())
        .and_then(|_| flux.write_all(b"\r\n"))
        .and_then(|_| flux.flush())
        .map_err(|e| Erreur::Reseau(format!("ecriture : {e}")))
}

/// Envoie un verbe et exige un code precis.
fn commande(flux: &mut Canal, ligne: &str, attendus: &[&str]) -> Result<String, Erreur> {
    ecrire(flux, ligne)?;
    let (code, texte) = reponse(flux)?;
    let ok = attendus.iter().any(|a| a.parse::<u16>() == Ok(code));
    if !ok {
        let verbe: String = ligne.split_whitespace().next().unwrap_or("").to_string();
        return Err(Erreur::Serveur(code, verbe, texte));
    }
    Ok(texte)
}

fn salutation(flux: &mut Canal, hote: &str) -> Result<(), Erreur> {
    let (code, texte) = reponse(flux)?;
    if code != 220 {
        return Err(Erreur::Serveur(code, "banniere".into(), texte));
    }
    let nom = hote.split('.').next().unwrap_or("localhost");
    commande(flux, &format!("EHLO {nom}"), &["250"])?;
    Ok(())
}

fn dialogue(
    mut flux: Canal,
    config: &Config,
    message: &Message,
    deja_salue: bool,
) -> Result<(), Erreur> {
    if deja_salue {
        // Apres STARTTLS, le serveur redemande EHLO.
        let nom = config.hote.split('.').next().unwrap_or("localhost");
        commande(&mut flux, &format!("EHLO {nom}"), &["250"])?;
    } else {
        salutation(&mut flux, &config.hote)?;
    }

    if !config.utilisateur.is_empty() {
        // AUTH PLAIN : \0 utilisateur \0 mot de passe, en base64.
        let jeton = format!("\0{}\0{}", config.utilisateur, config.mot_de_passe);
        let b64 = base64::engine::general_purpose::STANDARD.encode(jeton);
        commande(&mut flux, &format!("AUTH PLAIN {b64}"), &["235"])?;
    }

    let expediteur = if config.expediteur.is_empty() {
        message.de.clone()
    } else {
        config.expediteur.clone()
    };
    commande(&mut flux, &format!("MAIL FROM:<{expediteur}>"), &["250"])?;
    for destinataire in message.a.iter().chain(message.cc.iter()) {
        commande(
            &mut flux,
            &format!("RCPT TO:<{destinataire}>"),
            &["250", "251"],
        )?;
    }
    commande(&mut flux, "DATA", &["354"])?;

    let corps = assembler(config, message, &expediteur);
    for ligne in corps.split('\n') {
        // Point en debut de ligne : on le double (RFC 5321).
        if ligne.starts_with('.') {
            ecrire(&mut flux, &format!(".{ligne}"))?;
        } else {
            ecrire(&mut flux, ligne)?;
        }
    }
    commande(&mut flux, ".", &["250"])?;
    let _ = commande(&mut flux, "QUIT", &["221", "250"]);
    Ok(())
}

/// Construit le message complet : en-tetes, frontiere, corps, pieces jointes.
fn assembler(config: &Config, message: &Message, expediteur: &str) -> String {
    let frontiere = format!("=_dolibarr_rs_{}", horodatage());
    let nom = if config.nom_expediteur.is_empty() {
        String::new()
    } else {
        format!("=?UTF-8?B?{}?= ", b64(&config.nom_expediteur))
    };

    let mut sortie = String::new();
    sortie.push_str(&format!("From: {nom}<{expediteur}>\r\n"));
    sortie.push_str(&format!("To: {}\r\n", message.a.join(", ")));
    if !message.cc.is_empty() {
        sortie.push_str(&format!("Cc: {}\r\n", message.cc.join(", ")));
    }
    sortie.push_str(&format!("Subject: =?UTF-8?B?{}?=\r\n", b64(&message.sujet)));
    sortie.push_str("MIME-Version: 1.0\r\n");
    sortie.push_str("Date: ");
    sortie.push_str(&date_rfc5322());
    sortie.push_str("\r\n");

    if message.pieces.is_empty() && message.html.is_none() {
        sortie.push_str("Content-Type: text/plain; charset=UTF-8\r\n");
        sortie.push_str("Content-Transfer-Encoding: 8bit\r\n");
        sortie.push_str("\r\n");
        sortie.push_str(&message.texte);
        sortie.push_str("\r\n");
        return sortie;
    }

    sortie.push_str(&format!(
        "Content-Type: multipart/{}; boundary=\"{frontiere}\"\r\n\r\n",
        if message.pieces.is_empty() {
            "alternative"
        } else {
            "mixed"
        }
    ));

    // Partie texte + HTML (alternative), ou directement les pieces jointes.
    let sous_frontiere = format!("{frontiere}_alt");
    if message.pieces.is_empty() {
        sortie.push_str(&format!("--{frontiere}\r\n"));
        sortie.push_str(&alternative(message, &sous_frontiere));
    } else {
        sortie.push_str(&format!("--{frontiere}\r\n"));
        sortie.push_str("Content-Type: multipart/alternative; boundary=\"");
        sortie.push_str(&sous_frontiere);
        sortie.push_str("\"\r\n\r\n");
        sortie.push_str(&alternative(message, &sous_frontiere));
        for (nom_piece, contenu, mime) in &message.pieces {
            sortie.push_str(&format!("--{frontiere}\r\n"));
            sortie.push_str(&format!("Content-Type: {mime}; name=\"{nom_piece}\"\r\n"));
            sortie.push_str("Content-Transfer-Encoding: base64\r\n");
            sortie.push_str(&format!(
                "Content-Disposition: attachment; filename=\"{nom_piece}\"\r\n\r\n"
            ));
            sortie.push_str(&base64::engine::general_purpose::STANDARD.encode(contenu));
            sortie.push_str("\r\n");
        }
    }
    sortie.push_str(&format!("--{frontiere}--\r\n"));
    sortie
}

fn alternative(message: &Message, frontiere: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("--{frontiere}\r\n"));
    s.push_str("Content-Type: text/plain; charset=UTF-8\r\n");
    s.push_str("Content-Transfer-Encoding: 8bit\r\n\r\n");
    s.push_str(&message.texte);
    s.push_str("\r\n");
    if let Some(html) = &message.html {
        s.push_str(&format!("--{frontiere}\r\n"));
        s.push_str("Content-Type: text/html; charset=UTF-8\r\n");
        s.push_str("Content-Transfer-Encoding: 8bit\r\n\r\n");
        s.push_str(html);
        s.push_str("\r\n");
    }
    s.push_str(&format!("--{frontiere}--\r\n"));
    s
}

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

fn horodatage() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Une date au format RFC 5322 : « Tue, 03 Nov 2026 14:11:00 +0000 ».
///
/// Le calendrier est calcule ici : ajouter une dependance pour six nombres
/// n'aurait pas de sens, et une date fausse fait classer le message en
/// indesirable par la plupart des serveurs.
fn date_rfc5322() -> String {
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (jour_semaine, jour, mois, annee, h, m, s) = civil(secondes);
    format!(
        "{jour_semaine}, {jour:02} {} {annee} {h:02}:{m:02}:{s:02} +0000",
        mois_nom(mois)
    )
}

const JOURS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
const MOIS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn mois_nom(mois: u32) -> &'static str {
    MOIS[(mois.clamp(1, 12) - 1) as usize]
}

/// Convertit un temps Unix en (jour de la semaine, jour, mois, annee, h, m, s).
///
/// Algorithme de Howard Hinnant (`civil_from_days`) : exact pour toutes les
/// dates, sans table ni dependance. L'epoque Unix est un jeudi.
fn civil(secondes: u64) -> (&'static str, u32, u32, i64, u32, u32, u32) {
    let jours = (secondes / 86_400) as i64;
    let reste = secondes % 86_400;
    let sem = JOURS[(jours.rem_euclid(7)) as usize];

    let z = jours + 719_468;
    let ere = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - ere * 146_097;
    let annee_ere = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let annee = annee_ere + ere * 400;
    let jour_annee = doe - (365 * annee_ere + annee_ere / 4 - annee_ere / 100);
    let mp = (5 * jour_annee + 2) / 153;
    let jour = (jour_annee - (153 * mp + 2) / 5 + 1) as u32;
    let mois = (mp + if mp < 10 { 3 } else { -9 }) as u32;
    let annee = annee + i64::from(mois <= 2);

    (
        sem,
        jour,
        mois,
        annee,
        (reste / 3600) as u32,
        ((reste / 60) % 60) as u32,
        (reste % 60) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// Un faux serveur SMTP : il repond le minimum et rend ce qu'il a recu.
    fn faux_serveur(auth: bool) -> (String, u16, mpsc::Receiver<String>) {
        let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = ecoute.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (flux, _) = ecoute.accept().unwrap();
            let mut lecture = BufReader::new(flux.try_clone().unwrap());
            let mut ecriture = flux;
            let mut recu = String::new();
            let repondre = |e: &mut std::net::TcpStream, s: &str| {
                e.write_all(s.as_bytes()).unwrap();
                e.write_all(b"\r\n").unwrap();
                e.flush().unwrap();
            };
            repondre(&mut ecriture, "220 test.local ESMTP");
            let mut en_donnees = false;
            loop {
                let mut ligne = String::new();
                if lecture.read_line(&mut ligne).unwrap() == 0 {
                    break;
                }
                let l = ligne.trim_end().to_string();
                if en_donnees {
                    if l == "." {
                        en_donnees = false;
                        repondre(&mut ecriture, "250 2.0.0 Ok: queued");
                    } else {
                        recu.push_str(&l);
                        recu.push('\n');
                    }
                    continue;
                }
                recu.push_str(&format!("CMD {l}\n"));
                if l.starts_with("EHLO") || l.starts_with("HELO") {
                    repondre(&mut ecriture, "250-test.local");
                    repondre(&mut ecriture, "250 AUTH PLAIN LOGIN");
                } else if l.starts_with("AUTH") {
                    if auth {
                        repondre(&mut ecriture, "235 2.7.0 Authentication successful");
                    } else {
                        repondre(&mut ecriture, "535 5.7.8 Authentication failed");
                    }
                } else if l.starts_with("MAIL") || l.starts_with("RCPT") {
                    repondre(&mut ecriture, "250 2.1.0 Ok");
                } else if l == "DATA" {
                    en_donnees = true;
                    repondre(&mut ecriture, "354 End data with <CR><LF>.<CR><LF>");
                } else if l == "QUIT" {
                    repondre(&mut ecriture, "221 Bye");
                    break;
                } else {
                    repondre(&mut ecriture, "250 Ok");
                }
            }
            let _ = tx.send(recu);
        });
        (String::from("127.0.0.1"), port, rx)
    }

    fn config(port: u16) -> Config {
        Config {
            hote: "127.0.0.1".into(),
            port,
            utilisateur: "contact@exemple.fr".into(),
            mot_de_passe: "secret".into(),
            expediteur: "contact@exemple.fr".into(),
            nom_expediteur: "Pierre Angulaire".into(),
            securite: Securite::Aucune,
            delai: 5,
        }
    }

    #[test]
    fn le_message_part_entier() {
        let (hote, port, rx) = faux_serveur(true);
        let mut c = config(port);
        c.hote = hote;

        let mut message = Message::texte(
            "contact@exemple.fr",
            &["client@exemple.fr"],
            "Votre devis PR2611-0030",
            "Bonjour,\r\n\r\nVoici votre devis.\r\n",
        );
        message.html = Some("<p>Bonjour,<br>Voici votre devis.</p>".into());
        message.pieces = vec![(
            "PR2611-0030.pdf".into(),
            b"%PDF-1.4 test".to_vec(),
            "application/pdf".into(),
        )];

        envoyer(&c, &message).unwrap();
        let recu = rx.recv().unwrap();

        // Le dialogue, dans l'ordre.
        let commandes: Vec<&str> = recu
            .lines()
            .filter(|l| l.starts_with("CMD "))
            .map(|l| &l[4..])
            .collect();
        assert!(commandes[0].starts_with("EHLO"));
        assert!(commandes[1].starts_with("AUTH PLAIN "));
        assert_eq!(commandes[2], "MAIL FROM:<contact@exemple.fr>");
        assert_eq!(commandes[3], "RCPT TO:<client@exemple.fr>");
        assert_eq!(commandes[4], "DATA");

        // Le contenu : sujet encode, corps, piece jointe.
        assert!(recu.contains("Subject: =?UTF-8?B?"));
        assert!(recu.contains("Content-Type: multipart/mixed"));
        assert!(recu.contains("PR2611-0030.pdf"));
        assert!(recu.contains("application/pdf"));
        assert!(recu.contains("multipart/alternative"));
    }

    #[test]
    fn un_mot_de_passe_refuse_remonte_l_erreur() {
        let (hote, port, _rx) = faux_serveur(false);
        let mut c = config(port);
        c.hote = hote;
        let message = Message::texte("a@b.fr", &["c@d.fr"], "Test", "corps");
        match envoyer(&c, &message) {
            Err(Erreur::Serveur(535, verbe, _)) => assert_eq!(verbe, "AUTH"),
            autre => panic!("erreur attendue 535, obtenu {autre:?}"),
        }
    }

    #[test]
    fn sans_destinataire_on_refuse_avant_de_connecter() {
        let c = config(1);
        let mut message = Message::texte("a@b.fr", &[], "Test", "corps");
        message.a.clear();
        assert!(envoyer(&c, &message).is_err());
    }

    #[test]
    fn un_point_en_debut_de_ligne_est_double() {
        // RFC 5321 : sinon le serveur croirait la fin du message.
        let (hote, port, rx) = faux_serveur(true);
        let mut c = config(port);
        c.hote = hote;
        let message = Message::texte("a@b.fr", &["c@d.fr"], "Test", ".cache\r\nligne");
        envoyer(&c, &message).unwrap();
        let recu = rx.recv().unwrap();
        assert!(recu.contains("..cache"), "le point n'a pas ete double");
    }

    #[test]
    fn la_date_suit_le_calendrier() {
        // 1er janvier 2000 a 00:00 UTC : un samedi, valeur canonique.
        let (sem, jour, mois, annee, h, m, s) = civil(946_684_800);
        assert_eq!(
            (sem, jour, mois, annee, h, m, s),
            ("Sat", 1, 1, 2000, 0, 0, 0)
        );
        // L'epoque Unix : un jeudi.
        let (sem0, jour0, mois0, annee0, _, _, _) = civil(0);
        assert_eq!((sem0, jour0, mois0, annee0), ("Thu", 1, 1, 1970));
        // Une annee bissextile : le 29 fevrier 2024 existe.
        let (_, jour_bis, mois_bis, annee_bis, _, _, _) = civil(1_709_164_800);
        assert_eq!((jour_bis, mois_bis, annee_bis), (29, 2, 2024));
    }

    #[test]
    fn le_mode_se_deduit_du_port() {
        assert_eq!(Config::securite_pour(465), Securite::Directe);
        assert_eq!(Config::securite_pour(587), Securite::Starttls);
        assert_eq!(Config::securite_pour(25), Securite::Starttls);
    }
}
