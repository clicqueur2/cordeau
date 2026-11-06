//! Un jeu d'icones dessine a la main, en SVG inline.
//!
//! Pas de police d'icones, pas de fichier a telecharger, pas de dependance : un
//! trait de 1,6 px sur une grille de 24, et le texte qui suit. Un SVG en ligne
//! prend la couleur du texte (`currentColor`), donc il s'accorde tout seul avec
//! le survol et l'onglet actif.
//!
//! Les tracés sont des formes geometriques simples, ecrites ici. Les icones de
//! Noun Project sont sous licence CC-BY (attribution obligatoire, par auteur) :
//! on ne les recopie pas.

use maud::{html, Markup, PreEscaped};

/// Le dessin d'une icone, ou `None` si le nom est inconnu.
fn trace(nom: &str) -> Option<&'static str> {
    Some(match nom {
        "accueil" => "M3 11.2 12 4l9 7.2V20a1 1 0 0 1-1 1h-5v-6H10v6H4a1 1 0 0 1-1-1z",
        "tiers" => "M8 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zM2 21v-2a5 5 0 0 1 5-5h2a5 5 0 0 1 5 5v2\
                    M17 11a3 3 0 1 0 0-6M16 14h1a5 5 0 0 1 5 5v2",
        "produits" => "M3 8l9-4 9 4v8l-9 4-9-4zM3 8l9 4 9-4M12 12v8",
        "chantiers" => "M4 21V6h16v15M8 6V3h8v3M4 12h16M9 21v-5h6v5",
        "commerce" => "M3 4h2l2.5 11h10L20 7H6M9 20a1 1 0 1 0 0-2 1 1 0 0 0 0 2zM17 20a1 1 0 1 0 0-2 1 1 0 0 0 0 2z",
        "achats" => "M2 5h3l2 11h11M7 8h13l-2 7H8M6 20a1 1 0 1 0 0-2 1 1 0 0 0 0 2zM17 20a1 1 0 1 0 0-2 1 1 0 0 0 0 2z",
        "factures" => "M6 3h12v18l-3-2-3 2-3-2-3 2V3zM9 8h6M9 12h6M9 16h3",
        "tickets" => "M4 8a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2a2 2 0 0 0 0 4v2a2 2 0 0 1-2 2H6\
                      a2 2 0 0 1-2-2v-2a2 2 0 0 0 0-4zM10 6v12",
        "banques" => "M3 10 12 4l9 6M5 10v9M9 10v9M15 10v9M19 10v9M3 21h18",
        "comptabilite" => "M5 3h11l3 3v15H5zM9 8h6M9 12h6M9 16h4",
        "documents" => "M4 4h7l3 3h6v13H4zM4 10h16",
        "agenda" => "M4 6h16v15H4zM4 10h16M8 3v4M16 3v4M8 14h3M8 17h6",
        "communication" => "M3 6h18v12H3zM3 7l9 6 9-6",
        "rh" => "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21v-1a6 6 0 0 1 6-6h4a6 6 0 0 1 6 6v1",
        "outils" => "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM12 2v3M12 19v3M2 12h3M19 12h3\
                     M5 5l2 2M17 17l2 2M19 5l-2 2M7 17l-2 2",
        "recherche" => "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM16 16l5 5",
        "envoyer" => "M3 12l18-9-7 18-3-7z",
        "telecharger" => "M12 3v12M7 11l5 5 5-5M4 21h16",
        "valider" => "M4 13l5 5L20 6",
        "ajouter" => "M12 5v14M5 12h14",
        "alertes" => "M12 3l9 16H3zM12 9v4M12 16h.01",
        _ => return None,
    })
}

/// Une icone, dimensionnee pour le texte qui l'accompagne.
///
/// Un nom inconnu rend une chaine vide plutot qu'un carre gris : une absence
/// discrete vaut mieux qu'un dessin casse.
pub fn icone(nom: &str) -> Markup {
    let Some(d) = trace(nom) else {
        return html! {};
    };
    html! {
        svg .ic width="16" height="16" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="1.6" stroke-linecap="round"
            stroke-linejoin="round" aria-hidden="true" {
            (PreEscaped(format!("<path d=\"{d}\"/>")))
        }
    }
}

/// L'icone d'un module, a partir de sa cle de menu.
pub fn du_module(cle: &str) -> Markup {
    let nom = match cle {
        "accueil" | "home" => "accueil",
        "tiers" | "thirdparties" => "tiers",
        "produits" | "productspipeservices" => "produits",
        "chantiers" | "projects" => "chantiers",
        "devis" | "commercial" => "commerce",
        "achats" => "achats",
        "factures" | "menufinancial" => "factures",
        "tickets" => "tickets",
        "banques" | "menubankcash" => "banques",
        "comptabilite" | "menuaccountancy" => "comptabilite",
        "documents" => "documents",
        "agenda" => "agenda",
        "communication" => "communication",
        "rh" | "hrm" => "rh",
        "parametres" | "tools" => "outils",
        _ => return html! {},
    };
    icone(nom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_icone_connue_rend_un_svg() {
        let s = icone("accueil").into_string();
        assert!(s.starts_with("<svg"), "{s}");
        assert!(s.contains("currentColor"), "la couleur suit le texte : {s}");
        assert!(s.contains("aria-hidden"), "decorative : {s}");
    }

    #[test]
    fn une_icone_inconnue_ne_rend_rien() {
        assert_eq!(icone("licorne").into_string(), "");
        assert_eq!(du_module("module-bidon").into_string(), "");
    }

    #[test]
    fn chaque_trace_est_un_trait_ferme_ou_ouvert_valide() {
        for nom in [
            "accueil",
            "tiers",
            "produits",
            "chantiers",
            "commerce",
            "achats",
            "factures",
            "tickets",
            "banques",
            "comptabilite",
            "documents",
            "agenda",
            "communication",
            "rh",
            "outils",
            "recherche",
        ] {
            let d = trace(nom).unwrap_or_else(|| panic!("trace manquant : {nom}"));
            assert!(
                d.starts_with('M'),
                "{nom} doit commencer par un deplacement"
            );
            assert!(!d.contains('<'), "{nom} ne doit pas contenir de balise");
        }
    }
}
