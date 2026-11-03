//! Mise en page de l'interface, sur le modele de Dolibarr : onglets en haut,
//! sous-onglets sous le bandeau, puis le contenu.

use maud::{html, Markup, DOCTYPE};

const CSS: &str = r#"
:root {
  --ink:#1c1b19; --muted:#78736c; --line:#e6e2dc; --bg:#f7f6f3; --blanc:#fff;
  --accent:#8a5a2b; --accent-clair:#f4ede5;
  --vert:#2f6b45; --vert-clair:#e9f2ec;
  --rouge:#a33a2a; --rouge-clair:#fbecea;
  --bleu:#2f5d7c; --bleu-clair:#e8f0f5;
}
* { box-sizing:border-box; }
html { -webkit-text-size-adjust:100%; }
body { margin:0; color:var(--ink); background:var(--bg);
       font:15px/1.55 "Segoe UI",system-ui,-apple-system,sans-serif; }
a { color:var(--accent); text-decoration:none; }
a:hover { text-decoration:underline; }
:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }

/* --- onglets principaux ------------------------------------------------- */
header { background:var(--blanc); border-bottom:1px solid var(--line);
         display:flex; align-items:stretch; gap:28px; padding:0 24px; }
header .marque { font-weight:600; letter-spacing:.02em; display:flex; align-items:center;
                 white-space:nowrap; margin-right:8px; }
header .marque span { color:var(--accent); }
header nav { display:flex; gap:2px; flex:1; overflow-x:auto; }
header nav a { color:var(--ink); padding:16px 14px 14px; border-bottom:3px solid transparent;
               white-space:nowrap; font-size:14.5px; }
header nav a:hover { background:#faf8f5; text-decoration:none; }
header nav a.actif { border-bottom-color:var(--accent); color:var(--accent); font-weight:600; }
header form { display:flex; align-items:center; margin-left:auto; }
header input[type=search] { border:1px solid var(--line); border-right:none;
  border-radius:2px 0 0 2px; padding:7px 11px; font:inherit; width:230px; background:var(--blanc); }
header form button { border-radius:0 2px 2px 0; padding:7px 13px; }

/* --- sous-onglets ------------------------------------------------------- */
.onglets { background:#fbfaf8; border-bottom:1px solid var(--line); padding:0 32px; display:flex; gap:4px; }
.onglets a { padding:9px 14px; font-size:13.5px; color:var(--muted);
             border-bottom:2px solid transparent; }
.onglets a:hover { color:var(--ink); text-decoration:none; }
.onglets a.actif { color:var(--accent); border-bottom-color:var(--accent); font-weight:600; }

/* --- corps : menu de gauche + contenu ----------------------------------- */
.corps { display:flex; align-items:flex-start; }
.menu-gauche { width:230px; flex:0 0 230px; background:var(--blanc);
               border-right:1px solid var(--line); padding:12px 0 40px;
               position:sticky; top:0; align-self:flex-start;
               min-height:calc(100vh - 57px); }
.menu-gauche .module-titre { font-size:11px; text-transform:uppercase; letter-spacing:.09em;
                             color:var(--muted); padding:10px 18px 6px; font-weight:600; }
.menu-gauche a, .menu-gauche span.avenir { display:block; padding:7px 18px; font-size:14px;
                             color:var(--ink); border-left:3px solid transparent; }
.menu-gauche a.actif { background:var(--accent-clair); border-left-color:var(--accent);
                             color:var(--accent); font-weight:600; }
.menu-gauche a:hover { background:#faf8f5; text-decoration:none; }
.menu-gauche span.avenir { color:#b6b0a7; }
.menu-gauche hr { border:none; border-top:1px solid var(--line); margin:12px 16px; }
header nav a.inactif, header nav span.inactif { color:#b6b0a7; padding:16px 14px 14px;
                             border-bottom:3px solid transparent; white-space:nowrap; font-size:14.5px; }
main { flex:1; min-width:0; }

main { padding:0 0 80px; }
.contenu { padding:26px 32px 0; max-width:1000px; }
h1 { font-size:23px; font-weight:600; margin:0 0 6px; letter-spacing:-.01em; }
.sous-titre { color:var(--muted); margin:0 0 24px; font-size:14px; }
h2 { font-size:12px; text-transform:uppercase; letter-spacing:.1em; color:var(--muted);
     margin:34px 0 12px; font-weight:600; }

table { width:100%; border-collapse:collapse; background:var(--blanc); border:1px solid var(--line); }
th { text-align:left; font-weight:600; font-size:12.5px; color:var(--muted);
     padding:11px 14px; border-bottom:1px solid var(--line); background:#fbfaf8; white-space:nowrap; }
td { padding:11px 14px; border-bottom:1px solid var(--line); }
tbody tr:hover { background:#fcfbf9; }
tr:last-child td { border-bottom:none; }
tfoot td { border-top:2px solid var(--line); background:#fbfaf8; font-weight:600; }
.num { text-align:right; font-variant-numeric:tabular-nums; white-space:nowrap; }
.ref { font-variant-numeric:tabular-nums; letter-spacing:.01em; }

.cartes { display:grid; grid-template-columns:repeat(auto-fit,minmax(190px,1fr)); gap:14px; }
.carte { background:var(--blanc); border:1px solid var(--line); padding:18px 20px; }
.carte .libelle { font-size:11.5px; text-transform:uppercase; letter-spacing:.09em;
                  color:var(--muted); margin-bottom:6px; }
.carte .valeur { font-size:26px; font-weight:600; font-variant-numeric:tabular-nums; letter-spacing:-.02em; }
.carte .note { font-size:13px; color:var(--muted); margin-top:4px; }
.carte.alerte { border-color:#e7c9c3; background:#fdf7f5; }
.carte.alerte .valeur { color:var(--rouge); }

.actions { margin:22px 0; display:flex; gap:12px; align-items:center; flex-wrap:wrap; }
button, .bouton { font:inherit; padding:9px 16px; background:var(--accent); color:#fff;
  border:none; cursor:pointer; border-radius:2px; display:inline-block; }
button:hover, .bouton:hover { background:#7a4e24; text-decoration:none; }
.bouton.secondaire { background:var(--blanc); color:var(--ink); border:1px solid var(--line); }
.bouton.secondaire:hover { background:#f3f1ee; }
button.petit, .bouton.petit { padding:5px 11px; font-size:13.5px; }
input[type=text], input[type=number], input[type=email], select {
  font:inherit; padding:8px 10px; border:1px solid var(--line); background:var(--blanc); border-radius:2px; }

.pastille { display:inline-block; font-size:12.5px; padding:2px 9px; border-radius:11px;
  border:1px solid transparent; white-space:nowrap; }
.p-brouillon { background:#f0eeea; color:#6a655e; border-color:#e2ded8; }
.p-validee { background:var(--bleu-clair); color:var(--bleu); border-color:#cfdee8; }
.p-payee { background:var(--vert-clair); color:var(--vert); border-color:#cde3d5; }
.p-retard { background:var(--rouge-clair); color:var(--rouge); border-color:#efd3cd; }

.flash { background:var(--vert-clair); border:1px solid #cde3d5; color:var(--vert);
         padding:11px 15px; margin:0 0 22px; border-radius:2px; }
.erreur { background:var(--rouge-clair); border:1px solid #efd3cd; color:var(--rouge);
          padding:11px 15px; margin:16px 0; border-radius:2px; }
.vide { color:var(--muted); padding:26px 0; }
.vide a { font-weight:600; }

.fiche { display:grid; grid-template-columns:2fr 1fr; gap:26px; align-items:start; }
.fiche dl { margin:0; background:var(--blanc); border:1px solid var(--line); padding:6px 18px; }
.fiche dt { font-size:11.5px; text-transform:uppercase; letter-spacing:.08em; color:var(--muted); margin-top:12px; }
.fiche dd { margin:2px 0 12px; }
.total-fort { font-size:21px; font-weight:600; font-variant-numeric:tabular-nums; }

@media (max-width:900px) {
  header { padding:0 12px; gap:10px; flex-wrap:wrap; }
  header nav { order:3; width:100%; }
  header nav a { padding:10px 12px; }
  header form { margin-left:auto; }
  header input[type=search] { width:150px; }
  .onglets { padding:0 16px; overflow-x:auto; }
  .contenu { padding:20px 16px 0; }
  .fiche { grid-template-columns:1fr; }
  .corps { display:block; }
  .menu-gauche { width:auto; position:static; min-height:0; display:flex;
                 overflow-x:auto; border-right:none; border-bottom:1px solid var(--line);
                 padding:0 8px; }
  .menu-gauche .module-titre, .menu-gauche hr { display:none; }
  .menu-gauche a, .menu-gauche span.avenir { white-space:nowrap; border-left:none;
                 border-bottom:2px solid transparent; }
  .menu-gauche a.actif { border-bottom-color:var(--accent); }
}
"#;

/// Sous-onglets disponibles par section : `(cle, lien, libelle)`.
pub fn page(section: &str, onglet: &str, titre: &str, contenu: Markup) -> Markup {
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

    html! {
        (DOCTYPE)
        html lang="fr" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (titre) " — Pierre Angulaire" }
                style { (CSS) }
            }
            body {
                header {
                    span .marque { "Pierre " span { "Angulaire" } }
                    nav {
                        @for m in crate::menu::modules().into_iter().filter(|m| crate::menu_actif::est_actif(m.code) || crate::manifeste::est_declare(m.code)) {
                            a class=(if m.cle == module.cle { "actif" } else { "" })
                              href=(lien_module(m)) { (m.libelle) }
                        }
                    }
                    form method="get" action="/recherche" {
                        input type="search" name="q" placeholder="Rechercher…" aria-label="Recherche";
                        button type="submit" { "Chercher" }
                    }
                    @let entites = crate::entite::liste();
                    @if entites.len() > 1 {
                        form method="post" action="/entite" style="margin-left:8px" {
                            select name="entity" onchange="this.form.submit()" aria-label="Societe" {
                                @for (id, nom) in &entites {
                                    option value=(id) selected[*id == crate::entite::courante()] { (nom) }
                                }
                            }
                        }
                    } @else {
                        span style="margin-left:10px;color:#78736c;font-size:13.5px" {
                            (entites.first().map(|(_, n)| n.clone()).unwrap_or_default())
                        }
                    }
                    a .bouton.secondaire href="/compte" style="margin-left:10px;padding:7px 12px" { "Mon compte" }
                    form method="post" action="/deconnexion" style="margin-left:6px" {
                        button type="submit" class="secondaire" title="Se déconnecter" { "Quitter" }
                    }
                }
                div .corps {
                    aside .menu-gauche {
                        div .module-titre { (module.libelle) }
                        @for entree in module.entrees {
                            a class=(if entree.cle == actif { "actif" } else { "" })
                              href=(entree.lien) { (entree.libelle) }
                        }
                        hr;
                        div .module-titre { "Tous les modules" }
                        @for m in crate::menu::modules().into_iter().filter(|m| crate::menu_actif::est_actif(m.code) || crate::manifeste::est_declare(m.code)) {
                            a href=(lien_module(m)) { (m.libelle) }
                        }
                    }
                    main {
                        div .contenu { (contenu) }
                    }
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

pub fn pastille(statut: i16, paye: i16, en_retard: bool) -> Markup {
    if en_retard {
        html! { span .pastille.p-retard { "en retard" } }
    } else if paye == 1 || statut == 2 {
        html! { span .pastille.p-payee { "payée" } }
    } else if statut == 0 {
        html! { span .pastille.p-brouillon { "brouillon" } }
    } else if statut == 3 {
        html! { span .pastille.p-brouillon { "abandonnée" } }
    } else {
        html! { span .pastille.p-validee { "validée" } }
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
