//! Le moteur de listes et de saisies generiques.
//!
//! Dolibarr expose des centaines de petites tables (types de contact, modes de
//! livraison, categories...). Ecrire un ecran par table produirait quarante
//! fichiers presque identiques. Ici une **declaration** suffit : un code, un
//! libelle, les colonnes a montrer — et le moteur fait la liste, le formulaire,
//! la creation et la suppression.
//!
//! # Ce qui est declare, pas devine
//!
//! Le moteur ne devine jamais une colonne : une liste qu'on ne sait pas decrire
//! ne s'affiche pas. Une liste vide serait pire qu'une absence — elle ferait
//! croire a une table sans donnees.

use std::collections::HashMap;

use axum::extract::State;
use axum::response::Redirect;
use maud::{html, Markup};

use crate::{Erreur, Res};

/// Une liste en lecture seule : un module, une requete, des colonnes.
///
/// Ces modules existent dans Dolibarr et ont un ecran ; le noyau les affiche
/// sans les ecrire. C'est moins que Dolibarr, mais ce n'est pas rien : on voit
/// ce que la base contient, et l'ecriture se fait dans Dolibarr si besoin.
pub struct ListeSimple {
    pub code: &'static str,
    pub titre: &'static str,
    pub intro: &'static str,
    pub sql: &'static str,
    pub colonnes: &'static [&'static str],
}

pub(crate) const LISTES: &[ListeSimple] = &[
    ListeSimple {
        code: "Adherent",
        titre: "Adhérents",
        intro: "Adhérents enregistrés dans Dolibarr (llx_adherent).",
        sql: "SELECT rowid, ref, CONCAT(lastname, ' ', COALESCE(firstname, '')) AS nom, \
                     COALESCE(town, ''), COALESCE(email, ''), COALESCE(datefin, '') \
              FROM llx_adherent WHERE entity = :entity \
              ORDER BY rowid DESC LIMIT 200",
        colonnes: &["N°", "Réf.", "Nom", "Ville", "E-mail", "Adhésion jusqu'au"],
    },
    ListeSimple {
        code: "Don",
        titre: "Dons",
        intro: "Dons reçus (llx_don), lecture seule.",
        sql: "SELECT rowid, ref, DATE_FORMAT(datedon, '%Y-%m-%d'), \
                     CAST(amount AS CHAR), CAST(fk_statut AS CHAR) \
              FROM llx_don WHERE entity = :entity \
              ORDER BY datedon DESC, rowid DESC LIMIT 200",
        colonnes: &["N°", "Réf.", "Date", "Montant", "État"],
    },
    ListeSimple {
        code: "Reception",
        titre: "Réceptions",
        intro: "Réceptions de marchandises (llx_reception), lecture seule.",
        sql: "SELECT r.ref, DATE_FORMAT(r.date_creation, '%Y-%m-%d'), \
                     COALESCE(s.nom, ''), CAST(r.fk_statut AS CHAR) \
              FROM llx_reception r LEFT JOIN llx_societe s ON s.rowid = r.fk_soc \
              WHERE r.entity = :entity \
              ORDER BY r.date_creation DESC, r.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Fournisseur", "Statut"],
    },
    ListeSimple {
        code: "Categorie",
        titre: "Catégories",
        intro: "Catégories de produits, de tiers et de contacts (llx_categorie).",
        sql: "SELECT rowid, label, CAST(type AS CHAR), COALESCE(description, '') \
              FROM llx_categorie WHERE entity = :entity OR entity = 0 \
              ORDER BY label ASC LIMIT 200",
        colonnes: &["N°", "Libellé", "Type", "Description"],
    },
    ListeSimple {
        code: "ExpenseReport",
        titre: "Notes de frais",
        intro: "Notes de frais (llx_expensereport), lecture seule.",
        sql: "SELECT ref, DATE_FORMAT(date_debut, '%Y-%m-%d'), \
                     DATE_FORMAT(date_fin, '%Y-%m-%d'), \
                     CAST(total_ttc AS CHAR), CAST(fk_statut AS CHAR) \
              FROM llx_expensereport WHERE entity = :entity \
              ORDER BY date_debut DESC, rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Du", "Au", "Total TTC", "Statut"],
    },
    ListeSimple {
        code: "Holiday",
        titre: "Congés",
        intro: "Demandes de congés (llx_holiday), lecture seule.",
        sql: "SELECT ref, DATE_FORMAT(date_debut, '%Y-%m-%d'), \
                     DATE_FORMAT(date_fin, '%Y-%m-%d'), CAST(statut AS CHAR) \
              FROM llx_holiday WHERE entity = :entity \
              ORDER BY date_debut DESC, rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Du", "Au", "Statut"],
    },
    ListeSimple {
        code: "Mailing",
        titre: "Emailings",
        intro: "Campagnes d'e-mailing (llx_mailing), lecture seule.",
        sql: "SELECT titre, COALESCE(sujet, ''), DATE_FORMAT(date_creat, '%Y-%m-%d'), \
                     CASE statut WHEN 0 THEN 'Brouillon' WHEN 1 THEN 'Validé' \
                                 WHEN 2 THEN 'Envoyé' ELSE CAST(statut AS CHAR) END \
              FROM llx_mailing WHERE entity = :entity \
              ORDER BY date_creat DESC, rowid DESC LIMIT 200",
        colonnes: &["Titre", "Sujet", "Créé le", "Statut"],
    },
    ListeSimple {
        code: "Prelevement",
        titre: "Prélèvements",
        intro: "Bordereaux de prélèvement SEPA (llx_prelevement_bons), lecture seule.",
        sql: "SELECT p.ref, DATE_FORMAT(p.datec, '%Y-%m-%d'), \
                     CAST(p.amount AS CHAR), \
                     CASE p.statut WHEN 0 THEN 'Brouillon' WHEN 1 THEN 'Validé' \
                                   WHEN 2 THEN 'Déposé' WHEN 3 THEN 'Crédité' \
                                   WHEN 4 THEN 'Rejeté' ELSE CAST(p.statut AS CHAR) END \
              FROM llx_prelevement_bons p \
              WHERE p.entity = :entity \
              ORDER BY p.datec DESC, p.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Montant", "Statut"],
    },
    ListeSimple {
        code: "Ficheinter",
        titre: "Fiches d'intervention",
        intro: "Fiches d'intervention (llx_fichinter), lecture seule.",
        sql: "SELECT f.ref, DATE_FORMAT(f.datec, '%Y-%m-%d'), \
                     COALESCE(s.nom, ''), \
                     CASE f.fk_statut WHEN 0 THEN 'Brouillon' WHEN 1 THEN 'Validée' \
                                      WHEN 2 THEN 'Clôturée' ELSE CAST(f.fk_statut AS CHAR) END \
              FROM llx_fichinter f \
              LEFT JOIN llx_societe s ON s.rowid = f.fk_soc \
              WHERE f.entity = :entity \
              ORDER BY f.datec DESC, f.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Tiers", "Statut"],
    },
    ListeSimple {
        code: "Bookmark",
        titre: "Signets",
        intro: "Signets enregistrés par les utilisateurs (llx_bookmark).",
        sql: "SELECT title, url, DATE_FORMAT(dateb, '%Y-%m-%d'), \
                     COALESCE(u.login, '') \
              FROM llx_bookmark b \
              LEFT JOIN llx_user u ON u.rowid = b.fk_user \
              WHERE b.entity = :entity \
              ORDER BY b.position ASC, b.rowid ASC LIMIT 200",
        colonnes: &["Titre", "Adresse", "Créé le", "Utilisateur"],
    },
    ListeSimple {
        code: "Tax",
        titre: "Taux de TVA",
        intro: "Dictionnaire des taux de TVA (llx_c_tva) : c'est lui qui alimente \
                les factures, pas une liste figée dans le code.",
        sql: "SELECT rowid, code, CAST(taux AS CHAR), COALESCE(note, ''), \
                     CASE active WHEN 1 THEN 'actif' ELSE 'inactif' END \
              FROM llx_c_tva WHERE entity = :entity \
              ORDER BY taux DESC, code ASC LIMIT 200",
        colonnes: &["N°", "Code", "Taux", "Note", "État"],
    },
    ListeSimple {
        code: "Salaries",
        titre: "Salaires",
        intro: "Bulletins de salaire (llx_salary), lecture seule.",
        sql: "SELECT s.ref, DATE_FORMAT(s.datec, '%Y-%m-%d'), \
                     COALESCE(CONCAT(u.firstname, ' ', u.lastname), ''), \
                     CAST(s.amount AS CHAR), \
                     CASE s.paye WHEN 1 THEN 'Payé' ELSE 'À payer' END \
              FROM llx_salary s \
              LEFT JOIN llx_user u ON u.rowid = s.fk_user \
              WHERE s.entity = :entity \
              ORDER BY s.datec DESC, s.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Salarié", "Montant", "Paiement"],
    },
    ListeSimple {
        code: "Loan",
        titre: "Prêts",
        intro: "Emprunts et échéanciers (llx_loan), lecture seule.",
        sql: "SELECT rowid, label, CAST(capital AS CHAR), \
                     DATE_FORMAT(datestart, '%Y-%m-%d'), CAST(nbterm AS CHAR), \
                     CAST(rate AS CHAR), CAST(capital_position AS CHAR) \
              FROM llx_loan WHERE entity = :entity \
              ORDER BY datestart DESC, rowid DESC LIMIT 200",
        colonnes: &[
            "N°",
            "Libellé",
            "Capital",
            "Début",
            "Échéances",
            "Taux",
            "Capital restant",
        ],
    },
    ListeSimple {
        code: "ProductBatch",
        titre: "Lots et séries",
        intro: "Lots suivis (llx_product_lot), lecture seule.",
        sql: "SELECT l.rowid, l.batch, COALESCE(p.ref, ''), COALESCE(p.label, ''), \
                     COALESCE(DATE_FORMAT(l.eatby, '%Y-%m-%d'), ''), \
                     COALESCE(DATE_FORMAT(l.sellby, '%Y-%m-%d'), '') \
              FROM llx_product_lot l \
              LEFT JOIN llx_product p ON p.rowid = l.fk_product \
              WHERE l.entity = :entity \
              ORDER BY l.rowid DESC LIMIT 200",
        colonnes: &[
            "N°",
            "Lot",
            "Réf. produit",
            "Libellé",
            "À consommer avant",
            "À vendre avant",
        ],
    },
    ListeSimple {
        code: "Accounting",
        titre: "Écritures comptables",
        intro: "Table llx_accounting_bookkeeping : ce que Dolibarr y écrit. Vide tant \
                qu'aucun export n'a été validé — le grand livre de l'écran Comptabilité \
                est, lui, dérivé des documents.",
        sql: "SELECT numero_compte, COALESCE(label_compte, ''), \
                     COALESCE(label_operation, ''), DATE_FORMAT(doc_date, '%Y-%m-%d'), \
                     CAST(debit AS CHAR), CAST(credit AS CHAR) \
              FROM llx_accounting_bookkeeping WHERE entity = :entity \
              ORDER BY doc_date DESC, rowid DESC LIMIT 200",
        colonnes: &[
            "Compte",
            "Libellé compte",
            "Opération",
            "Date",
            "Débit",
            "Crédit",
        ],
    },
    ListeSimple {
        code: "Asset",
        titre: "Immobilisations",
        intro: "Biens immobilisés (llx_asset), lecture seule.",
        sql: "SELECT a.rowid, a.ref, a.label, DATE_FORMAT(a.date_acquisition, '%Y-%m-%d'), \
                     CAST(a.acquisition_value_ht AS CHAR), \
                     COALESCE(DATE_FORMAT(a.disposal_date, '%Y-%m-%d'), '') \
              FROM llx_asset a WHERE a.entity = :entity \
              ORDER BY a.date_acquisition DESC, a.rowid DESC LIMIT 200",
        colonnes: &["N°", "Réf.", "Libellé", "Acquis le", "Valeur HT", "Cédé le"],
    },
    ListeSimple {
        code: "Resource",
        titre: "Ressources",
        intro: "Ressources partagées : salles, véhicules, matériel (llx_resource).",
        sql: "SELECT rowid, ref, COALESCE(description, ''), COALESCE(town, ''), \
                     COALESCE(phone, ''), COALESCE(email, ''), \
                     CASE fk_statut WHEN 0 THEN 'inactif' ELSE 'actif' END \
              FROM llx_resource WHERE entity = :entity \
              ORDER BY ref ASC LIMIT 200",
        colonnes: &[
            "N°",
            "Réf.",
            "Description",
            "Ville",
            "Téléphone",
            "E-mail",
            "État",
        ],
    },
    ListeSimple {
        code: "PaymentByBankTransfer",
        titre: "Virements",
        intro: "Règlements par virement (llx_paiement, moyen de paiement VIR) : c'est \
                la matière du bordereau de virement.",
        sql: "SELECT p.ref, DATE_FORMAT(p.datep, '%Y-%m-%d'), \
                     CAST(p.amount AS CHAR), COALESCE(ba.label, ''), COALESCE(p.num_paiement, '') \
              FROM llx_paiement p \
              JOIN llx_c_paiement cp ON cp.id = p.fk_paiement \
              LEFT JOIN llx_bank b ON b.rowid = p.fk_bank \
              LEFT JOIN llx_bank_account ba ON ba.rowid = b.fk_account \
              WHERE p.entity = :entity AND cp.code = 'VIR' \
              ORDER BY p.datep DESC, p.rowid DESC LIMIT 200",
        colonnes: &["Réf.", "Date", "Montant", "Compte bancaire", "N° pièce"],
    },
];

/// Listes ajoutees par les modules declaratifs.
///
/// Un `RwLock` et non un `OnceLock` : chaque module ajoute les siennes, et le
/// premier arrive ne doit pas condamner les suivants. Chaque liste est fuitee
/// a l'enregistrement, donc les references rendues sont vraiment `'static`.
static LISTES_DECLAREES: std::sync::RwLock<Vec<&'static ListeSimple>> =
    std::sync::RwLock::new(Vec::new());

/// Ajoute des listes venues d'un module declaratif.
pub fn enregistrer_listes(listes: Vec<ListeSimple>) {
    let mut v = LISTES_DECLAREES.write().unwrap_or_else(|e| e.into_inner());
    for l in listes {
        if v.iter().any(|x| x.code.eq_ignore_ascii_case(l.code)) {
            continue;
        }
        v.push(Box::leak(Box::new(l)));
    }
}

/// Toutes les listes : celles du noyau, puis celles des modules.
pub fn liste_simple(code: &str) -> Option<&'static ListeSimple> {
    if let Some(l) = LISTES.iter().find(|l| l.code.eq_ignore_ascii_case(code)) {
        return Some(l);
    }
    LISTES_DECLAREES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|l| l.code.eq_ignore_ascii_case(code))
        .copied()
}

/// Affiche une liste en lecture seule, avec un lien de creation quand le
/// module en a un.
pub async fn liste_simple_page(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
) -> Res<Markup> {
    let liste = liste_simple(&code).ok_or_else(|| Erreur(format!("liste inconnue : {code}")))?;
    let lignes = db::divers::lignes_brutes(&app.pool, crate::entite::courante(), liste.sql)?;
    let creation = saisie(&code).is_some();

    let valeur = |r: &Vec<Option<String>>, i: usize| -> String {
        r.get(i).cloned().flatten().unwrap_or_default()
    };

    Ok(crate::ui::page(
        "listes",
        "liste",
        liste.titre,
        html! {
            h1 { (liste.titre) }
            p .sous-titre { (liste.intro) }
            @if creation {
                div .actions {
                    a .bouton href={ "/liste/" (code) "/nouveau" } {(crate::langue::tr("Nouveau")) }
                }
            }
            @if lignes.is_empty() {
                (crate::ui::vide("Aucune ligne.", None))
            } @else {
                table {
                    thead {
                        tr {
                            @for c in liste.colonnes { th { (c) } }
                        }
                    }
                    tbody {
                        @for r in &lignes {
                            tr {
                                @for (i, _) in liste.colonnes.iter().enumerate() {
                                    td {
                                        @if i == 0 && creation {
                                            a href={ "/liste/" (code) "/fiche/" (valeur(r, 0)) } {
                                                (valeur(r, 0))
                                            }
                                        } @else {
                                            (valeur(r, i))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            p style="color:#78736c;font-size:13px" {
                @if creation {(crate::langue::tr("Écran de création et de lecture. Le reste des champs se saisit dans Dolibarr."))
                } @else {(crate::langue::tr("Lecture seule : les écritures se font dans Dolibarr."))
                }
            }
        },
    ))
}

// ---------------------------------------------------------------------------
// Saisie simple : un formulaire de creation pour les modules que le noyau ne
// couvre pas encore en ecriture. Meme code que la liste : un module n'a qu'un
// identifiant.
// ---------------------------------------------------------------------------

/// Genre de champ, qui commande le controle de saisie du navigateur.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Sorte {
    Texte,
    Nombre,
    Date,
    Entier,
}

impl Sorte {
    fn type_html(self) -> &'static str {
        match self {
            Sorte::Texte => "text",
            Sorte::Nombre => "text",
            Sorte::Date => "date",
            Sorte::Entier => "number",
        }
    }

    /// Normalise une valeur saisie : les nombres se tapent en francais
    /// (espace insecable et virgule), MySQL attend un point.
    pub fn normaliser(self, v: &str) -> Option<String> {
        let v = v.trim();
        if v.is_empty() {
            return None;
        }
        match self {
            Sorte::Nombre | Sorte::Entier => Some(
                v.chars()
                    .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
                    .collect::<String>()
                    .replace(',', "."),
            ),
            _ => Some(v.to_string()),
        }
    }
}

/// Un champ du formulaire de creation.
pub struct ChampSaisie {
    /// Nom de la colonne. Il vient d'une table fermee, jamais de la requete.
    pub nom: &'static str,
    pub libelle: &'static str,
    pub sorte: Sorte,
    pub obligatoire: bool,
    /// Source d'une liste deroulante (`produits`, `types_adherent`...).
    pub choix: Option<&'static str>,
    /// Valeur proposee a la creation.
    pub defaut: &'static str,
}

/// Un formulaire de creation.
pub struct Saisie {
    pub code: &'static str,
    pub table: &'static str,
    pub champs: &'static [ChampSaisie],
    /// Colonnes posees d'office : (`colonne`, expression SQL). `{user}` est
    /// remplace par l'identifiant de l'utilisateur connecte.
    pub fixes: &'static [(&'static str, &'static str)],
    /// Reference a numeroter : (`colonne`, prefixe).
    pub reference: Option<(&'static str, &'static str)>,
    /// Cle primaire : `rowid` presque partout, `id` dans les dictionnaires.
    pub pk: &'static str,
    /// La table porte-t-elle une colonne `entity` ? Faux pour un dictionnaire
    /// national (pays, types d'evenement).
    pub entite: bool,
    /// La cle est-elle auto-incrementee ? Faux pour `llx_c_country`, seme avec
    /// des identifiants fixes par l'installateur Dolibarr.
    pub pk_auto: bool,
}

pub(crate) const fn c(
    nom: &'static str,
    libelle: &'static str,
    sorte: Sorte,
    obligatoire: bool,
) -> ChampSaisie {
    ChampSaisie {
        nom,
        libelle,
        sorte,
        obligatoire,
        choix: None,
        defaut: "",
    }
}

/// Un champ alimente par une liste deroulante.
pub(crate) const fn cc(
    nom: &'static str,
    libelle: &'static str,
    source: &'static str,
) -> ChampSaisie {
    ChampSaisie {
        nom,
        libelle,
        sorte: Sorte::Entier,
        obligatoire: true,
        choix: Some(source),
        defaut: "",
    }
}

pub(crate) const SAISIES: &[Saisie] = &[
    Saisie {
        code: "Loan",
        table: "llx_loan",
        champs: &[
            c("label", "Libellé", Sorte::Texte, true),
            c("capital", "Capital emprunté", Sorte::Nombre, true),
            c("rate", "Taux (%)", Sorte::Nombre, true),
            c("datestart", "Première échéance", Sorte::Date, false),
            c("dateend", "Dernière échéance", Sorte::Date, false),
            c("nbterm", "Nombre d'échéances", Sorte::Entier, false),
        ],
        fixes: &[("datec", "NOW()"), ("paid", "0"), ("active", "1")],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Asset",
        table: "llx_asset",
        champs: &[
            c("ref", "Référence", Sorte::Texte, true),
            c("label", "Libellé", Sorte::Texte, true),
            c("date_acquisition", "Date d'acquisition", Sorte::Date, true),
            c("date_start", "Début d'amortissement", Sorte::Date, true),
            c("acquisition_value_ht", "Valeur HT", Sorte::Nombre, false),
            c("qty", "Quantité", Sorte::Nombre, false),
        ],
        fixes: &[
            ("date_creation", "NOW()"),
            ("fk_user_creat", "{user}"),
            ("status", "0"),
        ],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "ProductBatch",
        table: "llx_product_lot",
        champs: &[
            cc("fk_product", "Produit", "produits"),
            c("batch", "Numéro de lot", Sorte::Texte, true),
            c("manufacturing_date", "Fabrication", Sorte::Date, false),
            c("eatby", "À consommer avant", Sorte::Date, false),
            c("sellby", "À vendre avant", Sorte::Date, false),
        ],
        fixes: &[],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Resource",
        table: "llx_resource",
        champs: &[
            c("ref", "Référence", Sorte::Texte, true),
            c("description", "Description", Sorte::Texte, false),
            c("town", "Ville", Sorte::Texte, false),
            c("phone", "Téléphone", Sorte::Texte, false),
            c("email", "E-mail", Sorte::Texte, false),
        ],
        fixes: &[("datec", "NOW()"), ("fk_statut", "1")],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Tax",
        table: "llx_c_tva",
        champs: &[
            c("code", "Code (ex. TVA20)", Sorte::Texte, true),
            c("taux", "Taux (%)", Sorte::Nombre, true),
            c("note", "Note", Sorte::Texte, false),
        ],
        // France : c'est le pays de la base. Un taux etranger se saisit dans
        // Dolibarr, avec son pays.
        fixes: &[("fk_pays", "1"), ("active", "1")],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Adherent",
        table: "llx_adherent",
        champs: &[
            cc("fk_adherent_type", "Type d'adhérent", "types_adherent"),
            c("lastname", "Nom", Sorte::Texte, true),
            c("firstname", "Prénom", Sorte::Texte, false),
            c("town", "Ville", Sorte::Texte, false),
            c("email", "E-mail", Sorte::Texte, false),
            c("datefin", "Adhésion jusqu'au", Sorte::Date, false),
        ],
        fixes: &[("morphy", "'phy'"), ("statut", "1"), ("public", "0")],
        reference: Some(("ref", "ADH")),
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Categorie",
        table: "llx_categorie",
        champs: &[
            c("label", "Libellé", Sorte::Texte, true),
            cc("type", "Type", "types_categorie"),
        ],
        fixes: &[("visible", "1"), ("fk_parent", "0")],
        reference: None,
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
    Saisie {
        code: "Don",
        table: "llx_don",
        champs: &[
            c("amount", "Montant", Sorte::Nombre, true),
            c("datedon", "Date du don", Sorte::Date, false),
            cc("fk_soc", "Donateur", "tiers"),
        ],
        fixes: &[
            ("fk_country", "1"),
            ("fk_user_author", "{user}"),
            ("fk_statut", "1"),
        ],
        reference: Some(("ref", "DON")),
        pk: "rowid",
        entite: true,
        pk_auto: true,
    },
];

/// Formulaires ajoutes par les modules declaratifs.
static SAISIES_DECLAREES: std::sync::RwLock<Vec<&'static Saisie>> =
    std::sync::RwLock::new(Vec::new());

/// Ajoute des formulaires venus d'un module declaratif.
pub fn enregistrer_saisies(saisies: Vec<Saisie>) {
    let mut v = SAISIES_DECLAREES.write().unwrap_or_else(|e| e.into_inner());
    for s in saisies {
        if v.iter().any(|x| x.code.eq_ignore_ascii_case(s.code)) {
            continue;
        }
        v.push(Box::leak(Box::new(s)));
    }
}

pub fn saisie(code: &str) -> Option<&'static Saisie> {
    if let Some(s) = SAISIES.iter().find(|s| s.code.eq_ignore_ascii_case(code)) {
        return Some(s);
    }
    SAISIES_DECLAREES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|s| s.code.eq_ignore_ascii_case(code))
        .copied()
}

/// Formulaire de creation.
pub async fn liste_saisie_form(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Res<Markup> {
    let s = saisie(&code)
        .ok_or_else(|| Erreur(format!("aucun formulaire de création pour : {code}")))?;
    let liste = liste_simple(&code).ok_or_else(|| Erreur(format!("liste inconnue : {code}")))?;
    // Le refus d'une saisie incomplete revient ici, indexe sur le champ, pour
    // que le message soit lisible au lieu d'une page d'erreur.
    let manquant = q
        .get("erreur")
        .and_then(|i| i.parse::<usize>().ok())
        .and_then(|i| s.champs.get(i))
        .map(|ch| format!("« {} » est obligatoire.", ch.libelle));

    // Les listes deroulantes sont chargees une fois par source.
    let mut cache: HashMap<&'static str, Vec<(String, String)>> = HashMap::new();
    let options: Vec<Option<Vec<(String, String)>>> = {
        let mut v = Vec::new();
        for ch in s.champs {
            match ch.choix {
                Some(src) => {
                    if !cache.contains_key(src) {
                        let liste =
                            db::divers::options_liste(&app.pool, crate::entite::courante(), src)?;
                        cache.insert(src, liste);
                    }
                    v.push(cache.get(src).cloned());
                }
                None => v.push(None),
            }
        }
        v
    };

    Ok(crate::ui::page(
        "listes",
        "liste",
        liste.titre,
        html! {
            h1 { "Nouveau — " (liste.titre) }
            @if let Some(m) = &manquant {
                p style="background:#fdf1ef;border-left:3px solid #a33;padding:10px 12px;margin:0 0 16px" {
                    (m)
                }
            }
            p .sous-titre {
                "Création dans "
                code { (s.table) }
                " : les valeurs sont contrôlées par la base, une contrainte manquante remonte ici au lieu d'être inventée."
            }
            form method="post" action={ "/liste/" (code) "/nouveau" } {
                table {
                    tbody {
                        @for (i, ch) in s.champs.iter().enumerate() {
                            tr {
                                th {
                                    (ch.libelle)
                                    @if ch.obligatoire { span style="color:#a33" { " *" } }
                                }
                                td {
                                    @if let Some(opts) = options.get(i).cloned().flatten() {
                                        select name=(ch.nom) {
                                            @for (valeur, libelle) in &opts {
                                                option value=(valeur) { (libelle) }
                                            }
                                        }
                                    } @else {
                                        input
                                            type=(ch.sorte.type_html())
                                            name=(ch.nom)
                                            value=(ch.defaut)
                                            required[ch.obligatoire];
                                        @if ch.sorte == Sorte::Nombre {
                                            p style="color:#78736c;font-size:12px;margin:6px 0 0" {(crate::langue::tr("Montant en euros, virgule ou point."))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Créer")) }
                    a .bouton href={ "/liste/" (code) } style="margin-left:8px" {(crate::langue::tr("Annuler")) }
                }
            }
        },
    ))
}

/// Enregistre la creation.
pub async fn liste_saisie_creer(
    State(app): State<crate::App>,
    axum::extract::Path(code): axum::extract::Path<String>,
    axum::Extension(courant): axum::Extension<crate::Courant>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let s = saisie(&code)
        .ok_or_else(|| Erreur(format!("aucun formulaire de création pour : {code}")))?;

    // Reference numerotee quand la table en exige une et que personne ne l'a
    // saisie (adherents, dons).
    let mut references: HashMap<&'static str, String> = HashMap::new();
    if let Some((colonne, prefixe)) = s.reference {
        references.insert(
            colonne,
            db::divers::suivant_ref(
                &app.pool,
                s.table,
                colonne,
                prefixe,
                crate::entite::courante(),
            )?,
        );
    }

    let mut valeurs: Vec<(String, Option<String>)> = Vec::new();
    for (i, ch) in s.champs.iter().enumerate() {
        let mut brut = f.get(ch.nom).map(String::as_str).unwrap_or("");
        if brut.trim().is_empty() {
            if let Some(r) = references.get(ch.nom) {
                brut = r;
            } else if !ch.defaut.is_empty() {
                brut = ch.defaut;
            }
        }
        let v = ch.sorte.normaliser(brut);
        if ch.obligatoire && v.is_none() {
            return Ok(Redirect::to(&format!("/liste/{code}/nouveau?erreur={i}")));
        }
        valeurs.push((ch.nom.to_string(), v));
    }

    // Une reference numerotee n'est pas forcement un champ du formulaire :
    // elle doit quand meme entrer en base (adherents, dons).
    for (colonne, valeur) in &references {
        if !valeurs.iter().any(|(c, _)| c == colonne) {
            valeurs.push((colonne.to_string(), Some(valeur.clone())));
        }
    }

    // Les colonnes posees d'office : soit un utilisateur, soit une expression
    // SQL fermee (constante du registre, jamais une saisie).
    let mut exprs: Vec<(&'static str, &'static str)> = Vec::new();
    let mut user: Option<i64> = None;
    for (col, expr) in s.fixes {
        if *expr == "{user}" {
            if user.is_none() {
                user = db::user::par_login(&app.pool, &courant.login)?.map(|u| u.rowid);
            }
            valeurs.push((col.to_string(), user.map(|u| u.to_string())));
        } else {
            exprs.push((col, expr));
        }
    }

    db::divers::inserer(
        &app.pool,
        cible(s),
        crate::entite::courante(),
        &valeurs,
        &exprs,
    )?;
    Ok(Redirect::to(&format!("/liste/{code}?ok=cree")))
}

/// La cible d'ecriture d'un formulaire declare.
fn cible(s: &Saisie) -> db::divers::Cible<'_> {
    db::divers::Cible {
        table: s.table,
        cle: s.pk,
        entite: s.entite,
        cle_auto: s.pk_auto,
    }
}

/// Met une valeur de la base sous une forme lisible dans un champ.
///
/// Les colonnes `double(24,8)` rendent « 3.50000000 » ; l'utilisateur doit voir
/// « 3,5 ». Les nombres passent par le decimal, jamais par un flottant.
fn valeur_affichable(ch: &ChampSaisie, v: &Option<String>) -> String {
    let brut = v.clone().unwrap_or_default();
    match ch.sorte {
        Sorte::Nombre | Sorte::Entier => {
            use std::str::FromStr;
            rust_decimal::Decimal::from_str(&brut)
                .map(|d| d.normalize().to_string())
                .unwrap_or(brut)
        }
        _ => brut,
    }
}

/// Fiche d'une ligne : les memes champs que la creation, remplis.
pub async fn fiche_saisie(
    State(app): State<crate::App>,
    axum::extract::Path((code, id)): axum::extract::Path<(String, i64)>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Res<Markup> {
    let s = saisie(&code).ok_or_else(|| Erreur(format!("aucune fiche pour : {code}")))?;
    let liste = liste_simple(&code).ok_or_else(|| Erreur(format!("liste inconnue : {code}")))?;

    let colonnes: Vec<&str> = s.champs.iter().map(|ch| ch.nom).collect();
    let ligne = db::divers::lire_ligne_par(
        &app.pool,
        cible(s),
        crate::entite::courante(),
        id,
        &colonnes,
    )?
    .ok_or_else(|| Erreur(format!("ligne {id} introuvable dans {}", s.table)))?;
    let valeurs: Vec<String> = s
        .champs
        .iter()
        .enumerate()
        .map(|(i, ch)| valeur_affichable(ch, ligne.get(i).unwrap_or(&None)))
        .collect();

    let manquant = q
        .get("erreur")
        .and_then(|i| i.parse::<usize>().ok())
        .and_then(|i| s.champs.get(i))
        .map(|ch| format!("« {} » est obligatoire.", ch.libelle));

    Ok(crate::ui::page(
        "listes",
        "liste",
        liste.titre,
        html! {
            h1 { (liste.titre) " — n° " (id.to_string()) }
            @if let Some(m) = &manquant {
                p style="background:#fdf1ef;border-left:3px solid #a33;padding:10px 12px;margin:0 0 16px" {
                    (m)
                }
            }
            form method="post" action={ "/liste/" (code) "/fiche/" (id.to_string()) } {
                table {
                    tbody {
                        @for (i, ch) in s.champs.iter().enumerate() {
                            tr {
                                th {
                                    (ch.libelle)
                                    @if ch.obligatoire { span style="color:#a33" { " *" } }
                                }
                                td {
                                    input
                                        type=(ch.sorte.type_html())
                                        name=(ch.nom)
                                        value=(valeurs.get(i).cloned().unwrap_or_default())
                                        required[ch.obligatoire];
                                }
                            }
                        }
                    }
                }
                div .actions {
                    button type="submit" {(crate::langue::tr("Enregistrer")) }
                    a .bouton href={ "/liste/" (code) } style="margin-left:8px" {(crate::langue::tr("Retour")) }
                }
            }
            form method="post"
                 action={ "/liste/" (code) "/fiche/" (id.to_string()) "/supprimer" }
                 onsubmit="return confirm('Supprimer définitivement cette ligne ?')"
                 style="margin-top:18px" {
                button type="submit"
                       style="background:#a33;color:#fff;border:0;padding:9px 16px;cursor:pointer" {(crate::langue::tr("Supprimer"))
                }
                span style="color:#78736c;font-size:13px;margin-left:10px" {
                    "Suppression directe dans " code { (s.table) } " : Dolibarr ne passera pas par là."
                }
            }
        },
    ))
}

/// Enregistre la modification d'une fiche.
pub async fn fiche_saisie_enregistrer(
    State(app): State<crate::App>,
    axum::extract::Path((code, id)): axum::extract::Path<(String, i64)>,
    axum::Form(f): axum::Form<HashMap<String, String>>,
) -> Res<Redirect> {
    let s = saisie(&code).ok_or_else(|| Erreur(format!("aucune fiche pour : {code}")))?;

    let mut valeurs: Vec<(String, Option<String>)> = Vec::new();
    for (i, ch) in s.champs.iter().enumerate() {
        let brut = f.get(ch.nom).map(String::as_str).unwrap_or("");
        let v = ch.sorte.normaliser(brut);
        if ch.obligatoire && v.is_none() {
            return Ok(Redirect::to(&format!(
                "/liste/{code}/fiche/{id}?erreur={i}"
            )));
        }
        valeurs.push((ch.nom.to_string(), v));
    }

    // Les colonnes posees d'office ne sont pas retouchees : une date de
    // creation ne se reecrit pas.
    db::divers::mettre_a_jour_par(
        &app.pool,
        cible(s),
        crate::entite::courante(),
        id,
        &valeurs,
        &[],
    )?;
    Ok(Redirect::to(&format!("/liste/{code}?ok=modifie")))
}

/// Supprime une fiche.
pub async fn fiche_saisie_supprimer(
    State(app): State<crate::App>,
    axum::extract::Path((code, id)): axum::extract::Path<(String, i64)>,
) -> Res<Redirect> {
    let s = saisie(&code).ok_or_else(|| Erreur(format!("aucune fiche pour : {code}")))?;
    db::divers::supprimer_ligne_par(&app.pool, cible(s), crate::entite::courante(), id)?;
    Ok(Redirect::to(&format!("/liste/{code}?ok=supprime")))
}
