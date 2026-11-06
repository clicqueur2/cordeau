//! Ecran de traduction : ce qui est deja traduit, et ce qui ne l'est pas.
//!
//! Dolibarr a `admin/translation.php`. Le principe est le meme, la mesure est
//! differente : ici le noyau est ecrit en francais, et chaque texte de
//! l'interface est cherche dans le catalogue Dolibarr. Un texte traduit est
//! donc un texte qu'on **retrouve** ; un texte absent reste en francais.
//!
//! L'ecran repond a une seule question utile : « si je bascule en anglais,
//! qu'est-ce qui reste en francais ? » — et il donne la liste, pas un
//! pourcentage tout seul. Un pourcentage sans la liste ne se corrige pas.
//!
//! # Ce que l'ecran ne fait pas
//!
//! Il n'ecrit pas de traduction. Les catalogues sont les fichiers `.lang` de
//! Dolibarr et les notres : les modifier depuis un formulaire obligerait a
//! gerer des ecritures concurrentes sur des fichiers versionnes. La ligne
//! exacte a ajouter s'obtient en ajoutant le texte dans `langs/noyau`.

use std::collections::HashMap;
use std::path::Path;

use axum::extract::{Query, State};
use maud::html;

use crate::{ui, App, Res};

/// Les textes en dur de l'interface, ou une chaine vide si le dossier est
/// introuvable.
///
/// Un inventaire vide n'est pas une erreur affichee comme telle : c'est une
/// mesure impossible, et l'ecran le dit.
fn textes_de_l_interface() -> Vec<String> {
    let racine = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    match i18n::ecrans::inventaire(&racine) {
        Ok(chaines) => chaines.into_iter().map(|c| c.texte).collect(),
        Err(_) => Vec::new(),
    }
}

/// La page.
pub async fn page(
    State(_app): State<App>,
    Query(q): Query<HashMap<String, String>>,
) -> Res<maud::Markup> {
    let active = crate::langue::langue();
    let examinee = q
        .get("langue")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| active.clone());

    let langues = crate::langue::langues_disponibles();
    let textes = textes_de_l_interface();
    let bilan = crate::langue::couverture(&examinee, &textes);
    let manquants = crate::langue::manquants(&examinee, &textes, 300);
    // Le chemin du catalogue est affiche : « 3 langues » n'est pas un bug quand
    // le dossier livre n'en contient que trois.
    let catalogue = crate::langue::racine_catalogue().display().to_string();

    Ok(ui::page(
        "parametres",
        "traduction",
        "Traduction",
        html! {
            h1 { (crate::langue::tr("Traduction")) }
            p .sous-titre {
                (crate::langue::tr("Le noyau est écrit en français. Chaque texte est cherché dans le catalogue Dolibarr : un texte trouvé est traduit, un texte absent reste en français. Cet écran montre lesquels."))
            }

            div .cartes {
                (ui::carte(
                    &crate::langue::tr("Langue active"),
                    &active,
                    Some(&crate::langue::tr("réglée par DOLIBARR_LANGUE")),
                    false,
                ))
                (ui::carte(
                    &crate::langue::tr("Textes de l'interface"),
                    &textes.len().to_string(),
                    Some(&crate::langue::tr("littéraux français repérés dans les écrans")),
                    false,
                ))
                (ui::carte(
                    &format!("{} — {} %", examinee, bilan.pourcentage()),
                    &format!("{} / {}", bilan.traduits, bilan.total),
                    Some(&crate::langue::tr("traduits dans cette langue")),
                    bilan.pourcentage() < 50,
                ))
                (ui::carte(
                    &crate::langue::tr("Langues disponibles"),
                    &langues.len().to_string(),
                    Some(&catalogue),
                    false,
                ))
            }

            @if textes.is_empty() {
                (ui::echec("Inventaire illisible : lancez le serveur depuis la racine du dépôt, sinon les écrans ne sont pas lisibles."))
            }

            h2 { (crate::langue::tr("Mesurer une langue")) }
            form method="get" action="/parametres/traduction" {
                label for="langue" { (crate::langue::tr("Langue")) } " "
                select name="langue" id="langue" style="min-width:220px" {
                    @for l in &langues {
                        option value=(l) selected[l == &examinee] { (l) }
                    }
                }
                " "
                button type="submit" class="petit" { (crate::langue::tr("Mesurer")) }
            }

            h2 { (crate::langue::tr("Ce qui resterait en français")) }
            @if manquants.is_empty() {
                @if bilan.total > 0 {
                    (ui::flash("Tous les textes de l'interface ont une traduction dans cette langue."))
                }
            } @else {
                p class="aide bloc" {
                    (crate::langue::tr("300 au plus, par ordre alphabétique. Pour en ajouter une, le texte se déclare dans langs/noyau : la clé est le texte français lui-même."))
                }
                table {
                    thead {
                        tr { th { (crate::langue::tr("Texte")) } }
                    }
                    tbody {
                        @for m in &manquants {
                            tr { td { (m.clone()) } }
                        }
                    }
                }
            }

            h2 { (crate::langue::tr("Comment ça marche")) }
            p class="aide" {
                (crate::langue::tr("La langue est globale au processus : elle se règle par DOLIBARR_LANGUE au démarrage. Un choix par utilisateur demanderait de faire descendre la langue dans chaque écran ; ce n'est pas fait, et cet écran ne fait pas semblant de le proposer."))
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn un_pourcentage_sans_liste_ne_se_corrige_pas() {
        // La liste des manquants est bornee : au-dela, elle ne sert plus a rien
        // et la page devient illisible.
        let bilan = crate::langue::Bilan {
            total: 10,
            traduits: 4,
        };
        assert_eq!(bilan.pourcentage(), 40);
    }
}
