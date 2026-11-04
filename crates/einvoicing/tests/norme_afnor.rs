use std::cell::RefCell;

use einvoicing::{
    Client, Configuration, Depot, Erreur, FormatDocument, Methode, Reponse, Requete, Transport,
};

/// Un transport qui ne sort pas : il note ce qu'on lui demande et rend ce
/// qu'on lui a préparé. C'est ce qui permet de vérifier l'échange entier sans
/// plateforme agréée et sans réseau.
struct Faux {
    reponses: RefCell<Vec<Reponse>>,
    vues: RefCell<Vec<Requete>>,
}

impl Faux {
    fn new(reponses: Vec<Reponse>) -> Self {
        Self {
            reponses: RefCell::new(reponses),
            vues: RefCell::new(Vec::new()),
        }
    }

    fn ok(corps: &str) -> Reponse {
        Reponse {
            code: 200,
            corps: corps.to_string(),
        }
    }
}

impl Transport for Faux {
    fn executer(&self, requete: &Requete) -> Result<Reponse, Erreur> {
        self.vues.borrow_mut().push(requete.clone());
        let mut restantes = self.reponses.borrow_mut();
        if restantes.is_empty() {
            return Err(Erreur::Transport("plus de reponse preparee".into()));
        }
        Ok(restantes.remove(0))
    }
}

fn configuration() -> Configuration {
    Configuration {
        racine: "https://pa.exemple.fr/".into(),
        identifiant: "id-client".into(),
        secret: "s3cr3t".into(),
        routes: Default::default(),
    }
}

fn depot() -> Depot {
    Depot {
        numero: "FA2611-0001".into(),
        siren_client: Some("123456789".into()),
        contenu: b"<xml/>".to_vec(),
        format: FormatDocument::Cii,
    }
}

/// L'authentification se fait une fois, pas une par facture.
#[test]
fn le_jeton_est_demande_une_seule_fois() {
    let faux = Faux::new(vec![
        Faux::ok(r#"{"access_token":"j1"}"#),
        Faux::ok(r#"{"identifiant":"P1"}"#),
        Faux::ok(r#"{"identifiant":"P2"}"#),
    ]);
    let client = Client::nouveau(configuration(), faux);

    assert_eq!(client.authentifier().unwrap(), "j1");
    assert_eq!(client.authentifier().unwrap(), "j1");

    client.emettre(&depot()).unwrap();
    client.emettre(&depot()).unwrap();

    let vues = client.transport().vues.borrow();
    let demandes_de_jeton = vues
        .iter()
        .filter(|r| r.url.ends_with("/oauth2/token"))
        .count();
    assert_eq!(demandes_de_jeton, 1, "un seul appel d'authentification");
}

/// L'émission porte le jeton, le numéro et le contenu encodé.
#[test]
fn l_emission_porte_le_document_et_le_jeton() {
    let faux = Faux::new(vec![
        Faux::ok(r#"{"access_token":"j1"}"#),
        Faux::ok(r#"{"identifiant":"PA-77","statut":"DEPOSE"}"#),
    ]);
    let client = Client::nouveau(configuration(), faux);

    let accuse = client.emettre(&depot()).expect("emission");
    assert_eq!(accuse.identifiant, "PA-77");
    assert_eq!(accuse.statut.as_deref(), Some("DEPOSE"));

    let vues = client.transport().vues.borrow();
    let envoi = vues.last().unwrap();
    assert_eq!(envoi.methode, Methode::Post);
    // La barre finale de la racine ne produit pas un double slash.
    assert_eq!(envoi.url, "https://pa.exemple.fr/api/v1/invoices");
    assert!(envoi
        .entetes
        .iter()
        .any(|(n, v)| n == "Authorization" && v == "Bearer j1"));
    assert!(envoi.corps.contains("FA2611-0001"));
    assert!(envoi.corps.contains("\"CII\""));
    // `<xml/>` en base64.
    assert!(envoi.corps.contains("PHhtbC8+"));
}

/// Un rejet de la plateforme remonte avec son code et son corps.
#[test]
fn un_refus_est_rendu_tel_quel() {
    let faux = Faux::new(vec![
        Faux::ok(r#"{"access_token":"j1"}"#),
        Reponse {
            code: 422,
            corps: r#"{"erreur":"SIRET invalide"}"#.into(),
        },
    ]);
    let client = Client::nouveau(configuration(), faux);

    match client.emettre(&depot()) {
        Err(Erreur::Refus { code, corps }) => {
            assert_eq!(code, 422);
            assert!(corps.contains("SIRET invalide"));
        }
        autre => panic!("attendu un refus, obtenu {autre:?}"),
    }
}

/// Le suivi de cycle de vie substitue l'identifiant dans la route.
#[test]
fn le_statut_est_demande_sur_le_bon_chemin() {
    let faux = Faux::new(vec![
        Faux::ok(r#"{"access_token":"j1"}"#),
        Faux::ok(r#"{"status":"RECU"}"#),
    ]);
    let client = Client::nouveau(configuration(), faux);

    let statut = client.statut("PA/77").expect("statut");
    assert_eq!(statut.code, "RECU");
    assert!(!statut.est_un_echec());

    let vues = client.transport().vues.borrow();
    // L'identifiant est echappe : une barre oblique ne casse pas la route.
    assert_eq!(
        vues.last().unwrap().url,
        "https://pa.exemple.fr/api/v1/invoices/PA%2F77/status"
    );
}

/// Un rejet est reconnu comme un echec, quel que soit le libelle exact.
#[test]
fn un_rejet_est_reconnu_comme_un_echec() {
    let faux = Faux::new(vec![
        Faux::ok(r#"{"access_token":"j1"}"#),
        Faux::ok(r#"{"code":"REJETEE","motif":"TVA absente"}"#),
    ]);
    let client = Client::nouveau(configuration(), faux);

    let statut = client.statut("P1").expect("statut");
    assert!(statut.est_un_echec());
    assert_eq!(statut.motif.as_deref(), Some("TVA absente"));
}

/// L'annuaire donne la plateforme du client.
#[test]
fn l_annuaire_rend_la_plateforme_du_client() {
    let faux = Faux::new(vec![
        Faux::ok(r#"{"access_token":"j1"}"#),
        Faux::ok(r#"{"siren":"123456789","platform":"Super PDP"}"#),
    ]);
    let client = Client::nouveau(configuration(), faux);

    let entree = client.annuaire("123456789").expect("annuaire");
    assert_eq!(entree.siren, "123456789");
    assert_eq!(entree.plateforme.as_deref(), Some("Super PDP"));
}

/// Une configuration incomplete est refusee avant tout appel reseau.
#[test]
fn une_configuration_incomplete_est_refusee() {
    let faux = Faux::new(vec![]);
    let mut config = configuration();
    config.secret = "   ".into();
    let client = Client::nouveau(config, faux);

    match client.authentifier() {
        Err(Erreur::Configuration(m)) => assert!(m.contains("secret")),
        autre => panic!("attendu une erreur de configuration, obtenu {autre:?}"),
    }
    assert!(
        client.transport().vues.borrow().is_empty(),
        "aucun appel ne doit partir"
    );
}

/// Un corps qui n'est pas du JSON attendu est signale, pas devine.
#[test]
fn un_corps_illisible_est_signale() {
    let faux = Faux::new(vec![Faux::ok("<html>maintenance</html>")]);
    let client = Client::nouveau(configuration(), faux);

    match client.authentifier() {
        Err(Erreur::Format(m)) => assert!(m.contains("jeton illisible")),
        autre => panic!("attendu une erreur de format, obtenu {autre:?}"),
    }
}
