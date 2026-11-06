//! La coque de l'interface : bandeau, menu de module, contenu.
//!
//! Tous les ecrans passent par `page()` : c'est le seul endroit ou se decide
//! ce qu'un utilisateur voit autour de son travail. La feuille de style vit
//! dans `theme.rs`, les icones dans `icones.rs`.

use maud::{html, Markup, DOCTYPE};

/// Sous-onglets disponibles par section : `(cle, lien, libelle)`.
/// La coque : bandeau, menu du module, contenu.
///
/// `section` designe le module (pour le menu de gauche), `onglet` l'entree
/// active du module, `titre` le titre de la page.
pub fn page(section: &str, onglet: &str, titre: &str, contenu: Markup) -> Markup {
    // Ecran sans module (connexion, erreur) : pas de menu, pas de selecteur de
    // societe, pas de bouton « quitter ». On ne montre pas le batiment a
    // quelqu'un qui n'a pas encore ouvert la porte.
    if section.is_empty() {
        return html! {
            (DOCTYPE)
            html lang="fr" {
                head {
                    meta charset="utf-8";
                    meta name="viewport" content="width=device-width, initial-scale=1";
                    meta name="color-scheme" content="light dark";
                    title { (titre) " — " (app::PRODUIT) }
                    style { (maud::PreEscaped(crate::theme::CSS)) }
                }
                body {
                    main id="contenu" {
                        div .contenu { (contenu) }
                    }
                }
            }
        };
    }

    let module = crate::menu::module_de(section);
    // Les relances sont une entree de Facturation : c'est le module qui porte
    // l'entree, pas la section d'ou vient l'appel.
    let actif = if section == "relances" {
        "relances"
    } else {
        onglet
    };

    let lien_module = |m: &'static crate::menu::Module| -> &'static str {
        m.entrees.first().map(|e| e.lien).unwrap_or("/")
    };
    let modules: Vec<&crate::menu::Module> = crate::menu::modules()
        .into_iter()
        .filter(|m| {
            crate::menu::depuis_base()
                || crate::menu_actif::est_actif(m.code)
                || crate::manifeste::est_declare(m.code)
        })
        .collect();
    let entites = crate::entite::liste();

    html! {
        (DOCTYPE)
        html lang="fr" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                title { (titre) " — " (app::PRODUIT) }
                style { (maud::PreEscaped(crate::theme::CSS)) }
            }
            body {
                // Premier element focusable : qui tabule peut sauter le menu.
                a .saut href="#contenu" { (crate::langue::tr("Aller au contenu")) }

                header {
                    a .marque href="/" title=(app::PRODUIT) {
                        span .sceau { "C" }
                        span { (app::PRODUIT) }
                    }

                    nav aria-label=(crate::langue::tr("Modules")) {
                        @for m in &modules {
                            a class=(if m.cle == module.cle { "actif" } else { "" })
                              href=(lien_module(m)) {
                                (crate::icones::du_module(m.cle))
                                (crate::langue::tr(m.libelle))
                            }
                        }
                    }

                    form .recherche method="get" action="/recherche" role="search" {
                        span .loupe { (crate::icones::icone("recherche")) }
                        input type="search" name="q" id="recherche" autocomplete="off"
                              placeholder=(crate::langue::tr("Rechercher…"))
                              aria-label=(crate::langue::tr("Rechercher"));
                        kbd { "/" }
                    }

                    // Le selecteur ne suffit pas : rien ne dit qu'il **filtre**. Une
                    // facture « absente » est alors cherchee dix minutes. Deux mots,
                    // et l'ambiguite tombe.
                    @if entites.len() > 1 {
                        form .societe method="post" action="/entite" {
                            select name="entity" onchange="this.form.submit()"
                                   aria-label=(crate::langue::tr("Société"))
                                   title=(crate::langue::tr("Toutes les listes sont filtrées sur cette société.")) {
                                @for (id, nom) in &entites {
                                    option value=(id) selected[*id == crate::entite::courante()] { (nom) }
                                }
                            }
                            span .mention { (crate::langue::tr("listes filtrées")) }
                        }
                    } @else {
                        span .muet .petite { (entites.first().map(|(_, n)| n.clone()).unwrap_or_default()) }
                    }

                    span .compte {
                        select id="style" aria-label=(crate::langue::tr("Style d'affichage"))
                               title=(crate::langue::tr("Style d'affichage"))
                               onchange="document.documentElement.setAttribute('data-style',this.value);try{localStorage.setItem('cordeau-style',this.value)}catch(e){}" {
                            option value="systeme" { (crate::langue::tr("Style système")) }
                            option value="futuriste" { (crate::langue::tr("Futuriste")) }
                            option value="minuit" { (crate::langue::tr("Minuit")) }
                            option value="sable" { (crate::langue::tr("Sable")) }
                            option value="classique" { (crate::langue::tr("Classique")) }
                        }
                        a .bouton.secondaire.petit href="/compte" { (crate::langue::tr("Mon compte")) }
                        form method="post" action="/deconnexion" {
                            button .fantome.petit type="submit"
                                   title=(crate::langue::tr("Se déconnecter")) {
                                (crate::langue::tr("Quitter"))
                            }
                        }
                    }
                }

                div .corps {
                    aside .menu-gauche {                        div .module-titre {
                            (crate::icones::du_module(module.cle))
                            (crate::langue::tr(module.libelle))
                        }
                        @for entree in module.entrees {
                            a class=(if entree.cle == actif { "actif" } else { "" })
                              href=(entree.lien) { (crate::langue::tr(entree.libelle)) }
                        }
                        hr;
                        div .module-titre { (crate::langue::tr("Tous les modules")) }
                        @for m in &modules {
                            a href=(lien_module(m)) {
                                (crate::icones::du_module(m.cle))
                                (crate::langue::tr(m.libelle))
                            }
                        }
                    }
                    main id="contenu" {
                        div .contenu { (contenu) }
                    }
                }
                // Le style choisi est une préférence locale, pas une donnée de la
                // base : il vit dans le navigateur et s'applique avant l'affichage.
                // Sans JavaScript, la page garde le style système — rien ne casse.
                script {
                    (maud::PreEscaped(
                        "(function(){try{var s=localStorage.getItem('cordeau-style')||'systeme';document.documentElement.dataset.style=s;var el=document.getElementById('style');if(el){el.value=s;el.addEventListener('change',function(){document.documentElement.dataset.style=el.value;localStorage.setItem('cordeau-style',el.value);});}}catch(e){}})();"
                    ))
                }
                // Quatre lignes de JavaScript, uniquement pour le raccourci « / ».
                // Sans elles, l'application fonctionne exactement pareil : la
                // recherche reste un formulaire. C'est un confort, pas une
                // dependance — le rendu, les actions et la navigation restent
                // entierement cote serveur.
                script {
                    (maud::PreEscaped(
                        "(function(){var c=['INPUT','TEXTAREA','SELECT'];document.addEventListener('keydown',function(e){if(e.key!=='/'||e.ctrlKey||e.metaKey)return;if(c.indexOf(document.activeElement.tagName)>=0)return;var f=document.getElementById('recherche');if(f){e.preventDefault();f.focus();}});})();"
                    ))
                }
            }
        }
    }
}

pub fn euros(v: rust_decimal::Decimal) -> String {
    let s = v.round_dp(2).normalize().to_string();
    let (entier, dec) = match s.split_once('.') {
        Some((e, d)) => (e.to_string(), format!("{d:0<2}")),
        None => (s, "00".to_string()),
    };
    let mut out = String::new();
    let negatif = entier.starts_with('-');
    let chiffres = entier.trim_start_matches('-');
    for (i, c) in chiffres.chars().enumerate() {
        if i > 0 && (chiffres.len() - i) % 3 == 0 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    format!("{}{out},{dec}\u{202f}€", if negatif { "-" } else { "" })
}

pub fn vide(message: &str, action: Option<(&str, &str)>) -> Markup {
    html! {
        p .vide {
            (message)
            @if let Some((lien, libelle)) = action {
                " " a href=(lien) { (libelle) }
            }
        }
    }
}

pub fn flash(message: &str) -> Markup {
    html! { p .flash { (message) } }
}

/// Le complément à afficher quand une liste est plus longue que ce qu'on charge.
///
/// Une liste tronquée qui se présente comme complète est un mensonge par
/// omission : sur la copie de production, l'écran des tiers annonçait « 500
/// tiers » pour 875 en base, et 375 fiches étaient introuvables sans que rien
/// ne le dise.
pub fn compte_tronque(affiches: usize, total: i64) -> Markup {
    let total = total.max(0) as usize;
    html! {
        @if total > affiches {
            " sur " (total)
            span .aide {
                " — seules les " (affiches) " plus récentes sont affichées ; affinez la recherche pour voir les autres"
            }
        }
    }
}

/// Un message d'echec, dans le style rouge de la coque.
pub fn echec(message: &str) -> Markup {
    html! { p .erreur { (message) } }
}

pub fn pastille(statut: i16, paye: i16, en_retard: bool) -> Markup {
    if en_retard {
        html! { span .pastille.p-retard {(crate::langue::tr("en retard")) } }
    } else if paye == 1 || statut == 2 {
        html! { span .pastille.p-payee {(crate::langue::tr("payée")) } }
    } else if statut == 0 {
        html! { span .pastille.p-brouillon { "brouillon" } }
    } else if statut == 3 {
        html! { span .pastille.p-brouillon {(crate::langue::tr("abandonnée")) } }
    } else {
        html! { span .pastille.p-validee {(crate::langue::tr("validée")) } }
    }
}

pub fn carte(libelle: &str, valeur: &str, note: Option<&str>, alerte: bool) -> Markup {
    html! {
        div .carte .alerte[alerte] {
            div .libelle { (libelle) }
            div .valeur { (valeur) }
            @if let Some(n) = note { div .note { (n) } }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_liste_tronquee_le_dit() {
        let rendu = compte_tronque(500, 875).into_string();
        assert!(rendu.contains("sur 875"), "{rendu}");
        assert!(rendu.contains("affinez"), "{rendu}");
    }

    #[test]
    fn une_liste_complete_ne_dit_rien() {
        assert_eq!(compte_tronque(17, 17).into_string().trim(), "");
        // Un total inconnu (0) ne doit pas faire croire a une troncature.
        assert_eq!(compte_tronque(17, 0).into_string().trim(), "");
    }
}
