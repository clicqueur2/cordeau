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
            html lang="fr" data-style=(crate::utilisateur::style()) {
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
        html lang="fr" data-style=(crate::utilisateur::style()) {
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

                // Le tiroir du menu, sur petit ecran : une case a cocher et deux
                // `label`. Pas de JavaScript, donc pas de menu qui reste ouvert
                // quand le script n'a pas charge.
                input type="checkbox" id="bascule-menu" .bascule;

                header {
                    label for="bascule-menu" .bascule-bouton
                          title=(crate::langue::tr("Menu")) {
                        span aria-hidden="true" { "☰" }
                        span .invisible { (crate::langue::tr("Menu")) }
                    }
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
                        // Un formulaire, pas un bout de JavaScript : le choix
                        // part au serveur et se range sur le compte.
                        form method="post" action="/style" .style-form {
                            select name="style" id="style"
                                   aria-label=(crate::langue::tr("Style d'affichage"))
                                   title=(crate::langue::tr("Style d'affichage"))
                                   onchange="this.form.submit()" {
                                @for (cle, nom, _) in crate::theme::STYLES {
                                    option value=(cle) selected[*cle == crate::utilisateur::style()] {
                                        (crate::langue::tr(nom))
                                    }
                                }
                            }
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
                    label for="bascule-menu" .voile title=(crate::langue::tr("Fermer le menu")) {}
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
                        "(function(){try{if(localStorage.getItem('cordeau-menu')==='reduit'){document.documentElement.dataset.menu='reduit';}}catch(e){}})();"
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

/// Un histogramme en SVG, sans bibliotheque ni JavaScript.
///
/// `serie` : `(etiquette, valeur)`, dans l'ordre d'affichage. Le maximum fixe la
/// hauteur ; une serie toute a zero affiche un cadre vide plutot que de diviser
/// par zero.
pub fn barres(serie: &[(String, f64)]) -> Markup {
    let max = serie.iter().map(|(_, v)| *v).fold(0.0f64, f64::max);
    let largeur = 720.0;
    let hauteur = 150.0;
    let marge_bas = 18.0;
    let n = serie.len().max(1) as f64;
    let pas = largeur / n;
    let largeur_barre = (pas * 0.62).max(4.0);

    html! {
        svg viewBox={ "0 0 720 168" } width="100%" height="168"
            role="img" aria-label=(crate::langue::tr("Chiffre d'affaires par mois")) {
            @for (i, (etiquette, valeur)) in serie.iter().enumerate() {
                @let h = if max > 0.0 { (valeur / max) * (hauteur - marge_bas) } else { 0.0 };
                @let x = i as f64 * pas + (pas - largeur_barre) / 2.0;
                @let y = hauteur - h;
                rect x=(format!("{x:.1}")) y=(format!("{y:.1}"))
                     width=(format!("{largeur_barre:.1}")) height=(format!("{h:.1}"))
                     rx="3" fill="currentColor" opacity="0.85" {
                    title { (etiquette) " : " (euros(rust_decimal::Decimal::from_f64_retain(*valeur).unwrap_or_default())) }
                }
                text x=(format!("{:.1}", i as f64 * pas + pas / 2.0)) y="165"
                     text-anchor="middle" font-size="10" fill="currentColor" opacity="0.55" {
                    (etiquette.get(5..).unwrap_or(etiquette))
                }
            }
        }
    }
}

/// Une barre de Gantt : ce qu'on montre, et de quand a quand.
pub struct BarreGantt {
    pub label: String,
    /// `AAAA-MM-JJ`, ou `AAAA-MM-JJ HH:MM:SS` (les taches de Dolibarr portent
    /// l'heure, les projets non).
    pub debut: String,
    pub fin: String,
    /// Avancement en pourcentage, 0 a 100.
    pub avancement: i32,
}

/// Jours depuis l'epoque civile, d'apres l'algorithme de Howard Hinnant.
///
/// Rend `None` si la date n'est pas lisible : une barre sans date n'a pas sa
/// place sur une echelle de temps, et l'inventer serait pire que l'oublier.
pub fn jours_depuis_epoque(date: &str) -> Option<i64> {
    let jour = date.trim().get(..10)?;
    let mut morceaux = jour.split('-');
    let annee: i64 = morceaux.next()?.parse().ok()?;
    let mois: i64 = morceaux.next()?.parse().ok()?;
    let j: i64 = morceaux.next()?.parse().ok()?;
    if !(1..=12).contains(&mois) || !(1..=31).contains(&j) {
        return None;
    }
    let a = if mois <= 2 { annee - 1 } else { annee };
    let ere = if a >= 0 { a } else { a - 399 } / 400;
    let annee_ere = a - ere * 400;
    let mois_0 = if mois > 2 { mois - 3 } else { mois + 9 };
    let jour_annee = (153 * mois_0 + 2) / 5 + j - 1;
    let jour_ere = annee_ere * 365 + annee_ere / 4 - annee_ere / 100 + jour_annee;
    Some(ere * 146_097 + jour_ere - 719_468)
}

/// Le diagramme de Gantt : une ligne par tache, une barre par duree.
///
/// L'echelle est commune a toutes les barres : c'est ce qui rend les
/// chevauchements lisibles, et donc les retards aussi.
pub fn gantt(barres: &[BarreGantt], aujourdhui: &str) -> Markup {
    let debut = barres
        .iter()
        .filter_map(|b| jours_depuis_epoque(&b.debut))
        .min();
    let fin = barres
        .iter()
        .filter_map(|b| jours_depuis_epoque(&b.fin))
        .max();
    let (Some(debut), Some(fin)) = (debut, fin) else {
        return html! { p.vide { (crate::langue::tr("Aucune date : pas d'echelle de temps.")) } };
    };
    // Une echelle d'un seul jour ne se divise pas : on ouvre d'un jour de
    // chaque cote pour que la barre ait une largeur visible.
    let etendue = (fin - debut).max(1) as f64;
    let largeur = 860.0;
    let colonne = 210.0;
    let hauteur_ligne = 26.0;
    let haut = 10.0;
    let hauteur = haut + hauteur_ligne * barres.len() as f64 + 24.0;
    let utile = largeur - colonne;
    let x = |jour: i64| colonne + (jour - debut) as f64 / etendue * utile;

    html! {
        div .defile {
            svg viewBox=(format!("0 0 {largeur:.0} {hauteur:.0}"))
                width="100%" height=(format!("{hauteur:.0}"))
                role="img"
                aria-label=(crate::langue::tr("Diagramme de Gantt des taches")) {
                @for (i, b) in barres.iter().enumerate() {
                    @let y = haut + i as f64 * hauteur_ligne;
                    @let d = jours_depuis_epoque(&b.debut);
                    @let f = jours_depuis_epoque(&b.fin);
                    text x="0" y=(format!("{:.0}", y + 15.0)) font-size="11.5"
                         fill="currentColor" { (b.label.clone()) }
                    @if let (Some(d), Some(f)) = (d, f) {
                        @let gauche = x(d);
                        @let droite = x(f.max(d)).max(x(d) + 2.0);
                        @let largeur_barre = (droite - gauche).max(2.0);
                        @let rempli = largeur_barre * (b.avancement.clamp(0, 100) as f64 / 100.0);
                        rect x=(format!("{gauche:.1}")) y=(format!("{:.1}", y + 5.0))
                             width=(format!("{largeur_barre:.1}")) height="14" rx="3"
                             fill="currentColor" opacity="0.16" {}
                        rect x=(format!("{gauche:.1}")) y=(format!("{:.1}", y + 5.0))
                             width=(format!("{rempli:.1}")) height="14" rx="3"
                             fill="currentColor" opacity="0.75" {
                            title { (b.label.clone()) " — " (b.avancement) " %" }
                        }
                    } @else {
                        text x=(format!("{:.0}", colonne + 4.0)) y=(format!("{:.0}", y + 15.0))
                             font-size="11" fill="currentColor" opacity="0.5" {
                            (crate::langue::tr("sans date"))
                        }
                    }
                }
                // La ligne du jour : ce qui est en retard se voit d'un coup d'oeil.
                @if let Some(j) = jours_depuis_epoque(aujourdhui) {
                    @if j >= debut && j <= fin {
                        line x1=(format!("{:.1}", x(j))) y1=(format!("{:.0}", haut))
                             x2=(format!("{:.1}", x(j))) y2=(format!("{:.0}", haut + hauteur_ligne * barres.len() as f64))
                             stroke="currentColor" stroke-width="1" opacity="0.45"
                             stroke-dasharray="3 3" {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests_gantt {
    use super::*;

    /// Trois dates connues, verifiees une fois pour toutes.
    #[test]
    fn les_jours_depuis_l_epoque_sont_justes() {
        assert_eq!(jours_depuis_epoque("1970-01-01"), Some(0));
        assert_eq!(jours_depuis_epoque("2000-01-01"), Some(10_957));
        assert_eq!(jours_depuis_epoque("2026-11-09"), Some(20_766));
    }

    /// L'heure ne compte pas pour une echelle en jours, et une date illisible
    /// ne devient jamais « aujourd'hui ».
    #[test]
    fn l_heure_est_ignoree_et_le_dechet_refuse() {
        assert_eq!(
            jours_depuis_epoque("2026-11-09 14:32:10"),
            jours_depuis_epoque("2026-11-09")
        );
        assert_eq!(jours_depuis_epoque("pas une date"), None);
        assert_eq!(jours_depuis_epoque("2026-13-01"), None);
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

/// La pastille d'un **devis**.
///
/// Les statuts d'un devis ne sont pas ceux d'une facture : 2 veut dire
/// « signée », pas « payée ». Afficher « payée » sur un devis fait douter de
/// tout le reste de l'ecran.
pub fn pastille_devis(statut: i16) -> Markup {
    let (classe, texte) = match statut {
        0 => ("p-brouillon", "brouillon"),
        2 => ("p-payee", "signée"),
        3 => ("p-brouillon", "abandonnée"),
        4 => ("p-validee", "facturée"),
        _ => ("p-validee", "ouverte"),
    };
    html! { span .pastille[classe == "p-payee"] .(classe) {(crate::langue::tr(texte)) } }
}

/// La pastille d'une **commande** : 2 = livrée, pas « payée ».
pub fn pastille_commande(statut: i16) -> Markup {
    let (classe, texte) = match statut {
        0 => ("p-brouillon", "brouillon"),
        2 => ("p-payee", "livrée"),
        3 => ("p-brouillon", "annulée"),
        _ => ("p-validee", "validée"),
    };
    html! { span .pastille[classe == "p-payee"] .(classe) {(crate::langue::tr(texte)) } }
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
