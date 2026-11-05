//! Ecrans de liste et de reglage qui remplacent les entrees vides du menu.
//!
//! Regle : un ecran qui n'affiche rien n'existe pas. Chaque page ci-dessous lit
//! de vraies donnees ou ecrit de vraies constantes Dolibarr.

mod contacts_ecran;
mod droits_ecran;
mod listes_ecran;
mod options_ecran;
mod reglages_ecran;
mod stock_ecran;
mod suivi_ecran;
mod systeme_ecran;
mod tiers_detail_ecran;

pub(crate) use contacts_ecran::*;
pub(crate) use droits_ecran::*;
pub(crate) use listes_ecran::*;
pub(crate) use options_ecran::*;
pub(crate) use reglages_ecran::*;
pub(crate) use stock_ecran::*;
pub(crate) use suivi_ecran::*;
pub(crate) use systeme_ecran::*;
pub(crate) use tiers_detail_ecran::*;

/// Un champ de reglage : constante Dolibarr, libelle, type, valeur par defaut.
pub struct Champ {
    pub cle: &'static str,
    pub libelle: &'static str,
    /// `text`, `nombre`, `oui_non` ou `textarea`.
    pub genre: &'static str,
    pub defaut: &'static str,
}

pub struct Section {
    pub cle: &'static str,
    pub titre: &'static str,
    pub intro: &'static str,
    pub champs: &'static [Champ],
}

pub const SECTIONS: &[Section] = &[
    Section {
        cle: "affichage",
        titre: "Affichage",
        intro: "Langue, devise et format de date par defaut. Ces constantes sont celles que lit Dolibarr.",
        champs: &[
            Champ { cle: "MAIN_LANG_DEFAULT", libelle: "Langue par defaut", genre: "text", defaut: "fr_FR" },
            Champ { cle: "MAIN_MONNAIE", libelle: "Devise", genre: "text", defaut: "EUR" },
            Champ { cle: "DOLIBARR_RS_FORMAT_DATE", libelle: "Format de date", genre: "text", defaut: "d/m/Y" },
        ],
    },
    Section {
        cle: "menus",
        titre: "Menus",
        intro: "Reglage de la barre de navigation.",
        champs: &[Champ {
            cle: "DOLIBARR_RS_MASQUER_MENU_HAUT",
            libelle: "Masquer la barre de modules",
            genre: "oui_non",
            defaut: "0",
        }],
    },
    Section {
        cle: "traduction",
        titre: "Traduction",
        intro: "Langue de l'interface. Les fichiers de traduction Dolibarr restent la reference.",
        champs: &[
            Champ { cle: "MAIN_LANG_DEFAULT", libelle: "Langue de l'interface", genre: "text", defaut: "fr_FR" },
            Champ { cle: "DOLIBARR_RS_LANGUE_SECONDAIRE", libelle: "Langue secondaire", genre: "text", defaut: "" },
        ],
    },
    Section {
        cle: "valeurs",
        titre: "Valeurs, filtres et tris par defaut",
        intro: "Taille des listes et ordre de tri.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_LIGNES_PAR_PAGE", libelle: "Lignes par page", genre: "nombre", defaut: "50" },
            Champ { cle: "DOLIBARR_RS_TRI_DEFAUT", libelle: "Tri par defaut", genre: "text", defaut: "date_desc" },
        ],
    },
    Section {
        cle: "widgets",
        titre: "Widgets",
        intro: "Cartes affichees sur le tableau de bord, separees par des virgules.",
        champs: &[Champ {
            cle: "DOLIBARR_RS_WIDGETS",
            libelle: "Cartes du tableau de bord",
            genre: "text",
            defaut: "facture, encaisse, reste, retard",
        }],
    },
    Section {
        cle: "alertes",
        titre: "Alertes",
        intro: "Seuils qui declenchent une alerte sur les impayes.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_ALERTE_RETARD_JOURS", libelle: "Alerte apres (jours de retard)", genre: "nombre", defaut: "5" },
            Champ { cle: "DOLIBARR_RS_ALERTE_EMAIL", libelle: "Destinataire des alertes", genre: "text", defaut: "" },
        ],
    },
    Section {
        cle: "securite",
        titre: "Securite",
        intro: "Cle d'API et longueur minimale des mots de passe.",
        champs: &[
            Champ { cle: "DOLIBARR_API_KEY", libelle: "Cle d'API (en-tete DOLAPIKEY)", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_PASSWORD_MIN", libelle: "Longueur minimale du mot de passe", genre: "nombre", defaut: "10" },
        ],
    },
    Section {
        cle: "limites",
        titre: "Limites et precision",
        intro: "Arrondis et decalage des listes.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_DECIMALES", libelle: "Decimales affichees", genre: "nombre", defaut: "2" },
            Champ { cle: "DOLIBARR_RS_LIMITE_EXPORT", libelle: "Lignes maximum a l'export", genre: "nombre", defaut: "5000" },
        ],
    },
    Section {
        cle: "pdf",
        titre: "Modeles de PDF",
        intro: "Marges du gabarit, en millimetres. Le gabarit les lit a chaque rendu.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_PDF_MODELE", libelle: "Modele de PDF (standard, epure, compact)", genre: "text", defaut: "standard" },
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_HAUT", libelle: "Marge haute", genre: "nombre", defaut: "14" },
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_BAS", libelle: "Marge basse", genre: "nombre", defaut: "34" },
            Champ { cle: "DOLIBARR_RS_PDF_MARGE_COTE", libelle: "Marge laterale", genre: "nombre", defaut: "10" },
        ],
    },
    Section {
        cle: "emails",
        titre: "Emails et notifications",
        intro: "Parametres SMTP. L'envoi est actif : serveur, compte, mot de passe, puis un essai pour verifier.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_SMTP_HOST", libelle: "Serveur SMTP", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMTP_PORT", libelle: "Port", genre: "nombre", defaut: "587" },
            Champ { cle: "DOLIBARR_RS_SMTP_UTILISATEUR", libelle: "Utilisateur", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_EMAIL_EXPEDITEUR", libelle: "Adresse d'expedition", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMTP_MOTDEPASSE", libelle: "Mot de passe SMTP", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMTP_SECURITE", libelle: "Securite (aucune, starttls, directe)", genre: "text", defaut: "" },
        ],
    },
    Section {
        cle: "sms",
        titre: "SMS",
        intro: "Fournisseur d'envoi de SMS. Aucun envoi n'est effectue pour l'instant.",
        champs: &[
            Champ { cle: "DOLIBARR_RS_SMS_FOURNISSEUR", libelle: "Fournisseur", genre: "text", defaut: "" },
            Champ { cle: "DOLIBARR_RS_SMS_EXPEDITEUR", libelle: "Expediteur", genre: "text", defaut: "" },
        ],
    },
    Section {
        cle: "divers",
        titre: "Divers",
        intro: "Pied de page imprime sur les documents.",
        champs: &[Champ {
            cle: "DOLIBARR_RS_PIED_PAGE",
            libelle: "Pied de page",
            genre: "textarea",
            defaut: "",
        }],
    },
];

pub(crate) fn section(cle: &str) -> Option<&'static Section> {
    SECTIONS.iter().find(|s| s.cle == cle)
}
