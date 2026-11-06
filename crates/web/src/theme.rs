//! Le système de style de l'interface, en un seul endroit.
//!
//! Trois idées, dans cet ordre.
//!
//! 1. **Clair, net, contemporain.** Fond gris froid très clair, surfaces
//!    blanches, une seule couleur d'action (indigo), et le reste en niveaux de
//!    gris. Rien de décoratif qui ne porte une information.
//!
//! 2. **Tout passe par des jetons.** Aucune couleur, aucune ombre, aucun rayon
//!    n'est écrit dans un écran. C'est ce qui permet le mode sombre sans
//!    reprendre une seule page, et c'est ce qui a manqué longtemps : 220 styles
//!    en ligne échappaient à la palette.
//!
//! 3. **Dense, mais respirant.** Un logiciel de gestion affiche beaucoup de
//!    chiffres : interlignes serrés, nombres en chasse fixe, titres courts. Le
//!    confort vient de l'alignement et du contraste, pas du vide.
//!
//! Les noms de classes existants sont conservés (`aide`, `petit`, `secondaire`,
//! `champ-court`…), parce que soixante écrans les emploient. Ce fichier les
//! redessine, il ne les renomme pas.

/// La feuille de style complète, servie dans chaque page.
pub(crate) const CSS: &str = r#"
/* ===== jetons ========================================================== */
:root {
  /* Encres : quatre niveaux, pas plus. */
  --encre: #0f1729;
  --encre-douce: #33415c;
  --muet: #64748b;
  --pale: #94a3b8;

  /* Surfaces */
  --papier: #f7f8fc;
  --surface: #ffffff;
  --surface-2: #fafbfe;
  --surface-3: #f1f4fa;

  /* Traits */
  --trait: #e7eaf2;
  --trait-fort: #d6dbe7;

  /* Action : un seul indigo, du plus clair au plus soutenu */
  --accent: #4f46e5;
  --accent-fonce: #4338ca;
  --accent-clair: #eef0ff;
  --accent-trait: #d7d9fb;
  --anneau: rgba(99, 102, 241, .35);

  /* États */
  --vert: #067a5b;
  --vert-clair: #e8faf3;
  --vert-trait: #b7ecd9;
  --rouge: #d92d20;
  --rouge-clair: #fef3f2;
  --rouge-trait: #fbd5d1;
  --bleu: #175cd3;
  --bleu-clair: #eff6ff;
  --bleu-trait: #cfe0fb;
  --ambre: #b54708;
  --ambre-clair: #fffaeb;
  --ambre-trait: #fde7b8;

  /* Formes */
  --rayon: 7px;
  --rayon-lg: 11px;
  --ombre-1: 0 1px 2px rgba(16, 24, 40, .05);
  --ombre-2: 0 8px 24px rgba(16, 24, 40, .08), 0 1px 2px rgba(16, 24, 40, .04);
  --ombre-3: 0 24px 48px rgba(16, 24, 40, .14), 0 2px 6px rgba(16, 24, 40, .06);
  --duree: .16s;
  --courbe: cubic-bezier(.22, .61, .36, 1);

  /* Rythme : 4 / 8 / 12 / 16 / 24 / 32 */
  --e1: 4px; --e2: 8px; --e3: 12px; --e4: 16px; --e5: 24px; --e6: 32px;
}

/* ===== base ============================================================ */
*, *::before, *::after { box-sizing: border-box; }
html { -webkit-text-size-adjust: 100%; scroll-behavior: smooth; }
body {
  margin: 0;
  color: var(--encre);
  background: var(--papier);
  font: 14.5px/1.55 "Inter", "Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, sans-serif;
  font-feature-settings: "kern" 1, "calt" 1, "cv11" 1;
  -webkit-font-smoothing: antialiased;
}
a { color: var(--accent); text-decoration: none; transition: color var(--duree) var(--courbe); }
a:hover { color: var(--accent-fonce); text-decoration: none; }
:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 4px; }
::selection { background: var(--accent-trait); color: var(--encre); }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { transition-duration: 0ms !important; animation-duration: 0ms !important; scroll-behavior: auto !important; }
}

.saut {
  position: absolute; left: -9999px; top: 0; z-index: 100;
  background: var(--accent); color: #fff; padding: 10px 16px; border-radius: var(--rayon);
}
.saut:focus { left: 12px; top: 12px; }

/* ===== icônes ========================================================== */
.ic { width: 16px; height: 16px; vertical-align: -3px; margin-right: 7px; flex: 0 0 auto; }
.loupe { position: absolute; left: 11px; top: 50%; transform: translateY(-50%); display: flex; color: var(--pale); }
.loupe .ic { width: 15px; height: 15px; margin: 0; }

/* ===== bandeau ========================================================= */
header {
  position: sticky; top: 0; z-index: 30;
  display: flex; align-items: center; gap: var(--e3);
  padding: 0 18px; min-height: 60px;
  background: color-mix(in srgb, var(--surface) 80%, transparent);
  backdrop-filter: saturate(180%) blur(14px);
  border-bottom: 1px solid var(--trait);
}
header .marque {
  display: flex; align-items: center; gap: 9px;
  font-weight: 680; letter-spacing: -.02em; white-space: nowrap;
  font-size: 15px; color: var(--encre);
  margin-right: var(--e2);
}
header .marque:hover { text-decoration: none; }
header .marque .sceau {
  display: grid; place-items: center; width: 28px; height: 28px;
  border-radius: 8px; color: #fff; font-size: 14px; font-weight: 700;
  background: linear-gradient(135deg, #6366f1 0%, #4f46e5 45%, #7c3aed 100%);
  box-shadow: 0 2px 6px rgba(79, 70, 229, .35), inset 0 1px 0 rgba(255, 255, 255, .3);
}
header .marque .suffixe { color: var(--pale); font-weight: 500; }

/* Onglets de module : pastilles arrondies, pas des soulignés. */
header nav { display: flex; gap: 2px; flex: 1; overflow-x: auto; scrollbar-width: none; }
header nav::-webkit-scrollbar { display: none; }
header nav a {
  display: inline-flex; align-items: center; gap: 7px;
  color: var(--muet); padding: 7px 11px; border-radius: 8px;
  white-space: nowrap; font-size: 13.5px; font-weight: 500;
  transition: background var(--duree) var(--courbe), color var(--duree) var(--courbe);
}
header nav a svg { width: 16px; height: 16px; opacity: .7; }
header nav a:hover { background: var(--surface-3); color: var(--encre); }
header nav a.actif { background: var(--accent-clair); color: var(--accent); font-weight: 600; }
header nav a.actif svg { opacity: 1; }
header nav a.inactif, header nav span.inactif {
  color: var(--pale); padding: 7px 11px; white-space: nowrap; font-size: 13.5px;
}

header form.recherche { display: flex; align-items: center; margin: 0; position: relative; }
header input[type=search] {
  font: inherit; font-size: 13.5px; color: var(--encre);
  padding: 8px 12px 8px 32px; width: 240px;
  border: 1px solid var(--trait); border-radius: 9px;
  background: var(--surface-3);
  transition: border-color var(--duree), background var(--duree), box-shadow var(--duree), width var(--duree);
}
header input[type=search]:focus {
  background: var(--surface); border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--anneau); width: 300px; outline: none;
}
header input[type=search]::placeholder { color: var(--pale); }
header form.recherche button { display: none; }
header form.recherche kbd {
  position: absolute; right: 9px; font: 600 10.5px/1 "Segoe UI", system-ui, sans-serif;
  color: var(--pale); background: var(--surface); border: 1px solid var(--trait);
  border-radius: 5px; padding: 2.5px 5px; pointer-events: none;
}

header form.societe { display: flex; align-items: center; gap: 6px; margin: 0; }
header form.societe select {
  font: inherit; font-size: 13px; padding: 7px 9px;
  border: 1px solid var(--trait); border-radius: 8px;
  background: var(--surface); color: var(--encre); max-width: 210px;
  font-weight: 550;
}
header form.societe .mention { color: var(--pale); font-size: 11px; white-space: nowrap; }
header .compte { display: flex; align-items: center; gap: 6px; margin: 0; }
header .compte form { margin: 0; }

/* ===== sous-onglets ==================================================== */
.onglets {
  display: flex; gap: 4px; padding: 0 var(--e6);
  background: var(--surface); border-bottom: 1px solid var(--trait);
  overflow-x: auto; scrollbar-width: none;
}
.onglets::-webkit-scrollbar { display: none; }
.onglets a { padding: 10px 12px; font-size: 13.5px; color: var(--muet); border-bottom: 2px solid transparent; white-space: nowrap; }
.onglets a:hover { color: var(--encre); background: var(--surface-2); }
.onglets a.actif { color: var(--accent); border-bottom-color: var(--accent); font-weight: 600; }

/* ===== corps =========================================================== */
.corps { display: flex; align-items: flex-start; }
.menu-gauche {
  width: 236px; flex: 0 0 236px;
  position: sticky; top: 60px; align-self: flex-start;
  height: calc(100vh - 60px); overflow-y: auto;
  background: var(--surface); border-right: 1px solid var(--trait);
  padding: var(--e3) 10px var(--e6);
}
.menu-gauche .module-titre {
  display: flex; align-items: center; gap: 7px;
  font-size: 10.5px; text-transform: uppercase; letter-spacing: .08em;
  color: var(--pale); padding: 10px 8px 7px; font-weight: 700;
}
.menu-gauche .module-titre svg { width: 14px; height: 14px; margin-right: 2px; }
.menu-gauche a, .menu-gauche span.avenir {
  display: flex; align-items: center; gap: 8px;
  padding: 7px 9px; font-size: 13.5px; color: var(--encre-douce);
  border-radius: 8px; margin: 1px 0; font-weight: 500;
  transition: background var(--duree) var(--courbe), color var(--duree) var(--courbe);
}
.menu-gauche a svg, .menu-gauche span.avenir svg { width: 15px; height: 15px; opacity: .55; flex: 0 0 15px; margin-right: 2px; }
.menu-gauche a:hover { background: var(--surface-3); color: var(--encre); }
.menu-gauche a.actif { background: var(--accent-clair); color: var(--accent); font-weight: 600; }
.menu-gauche a.actif svg { opacity: 1; }
.menu-gauche span.avenir { color: var(--pale); }
.menu-gauche span.avenir em { font-style: normal; font-size: 10.5px; color: var(--pale); margin-left: auto; }
.menu-gauche hr { border: none; border-top: 1px solid var(--trait); margin: var(--e3) 4px; }

main { flex: 1; min-width: 0; padding: 0 0 80px; }
.contenu { padding: var(--e5) var(--e6) 0; max-width: 1180px; }

/* ===== titres et texte ================================================= */
h1 {
  font-size: 27px; font-weight: 700; letter-spacing: -.03em;
  margin: 0 0 5px; line-height: 1.18;
}
.sous-titre { color: var(--muet); margin: 0 0 var(--e5); font-size: 14px; }
.sous-titre strong { color: var(--encre-douce); font-weight: 600; }
h2 {
  font-size: 11.5px; text-transform: uppercase; letter-spacing: .08em;
  color: var(--pale); margin: var(--e6) 0 var(--e3); font-weight: 700;
}
h3 { font-size: 15.5px; font-weight: 650; letter-spacing: -.01em; margin: var(--e5) 0 var(--e2); }
p { margin: 0 0 var(--e3); }

/* ===== tableaux ======================================================== */
table {
  width: 100%; border-collapse: separate; border-spacing: 0;
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); overflow: hidden; box-shadow: var(--ombre-1);
  font-size: 14px;
}
thead th {
  position: sticky; top: 60px; z-index: 2;
  text-align: left; font-weight: 600; font-size: 10.8px;
  text-transform: uppercase; letter-spacing: .07em; color: var(--muet);
  padding: 10px 14px; border-bottom: 1px solid var(--trait);
  background: var(--surface-2); white-space: nowrap;
}
td { padding: 10px 14px; border-bottom: 1px solid var(--trait); vertical-align: middle; }
tbody tr { transition: background var(--duree) var(--courbe); }
tbody tr:hover { background: var(--surface-2); }
tbody tr:last-child td { border-bottom: none; }
tfoot td { border-top: 1px solid var(--trait-fort); background: var(--surface-2); font-weight: 650; }
.num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
.ref { font-variant-numeric: tabular-nums; letter-spacing: -.01em; font-size: 13.5px; font-weight: 550; }
td .aide { display: block; margin-top: 2px; }
td a { font-weight: 550; }

/* ===== cartes et indicateurs =========================================== */
.cartes { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: var(--e3); }
.carte {
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 16px 18px; box-shadow: var(--ombre-1);
  transition: box-shadow var(--duree) var(--courbe), border-color var(--duree) var(--courbe);
}
.carte:hover { box-shadow: var(--ombre-2); border-color: var(--trait-fort); }
.carte .libelle {
  font-size: 10.8px; text-transform: uppercase; letter-spacing: .07em;
  color: var(--muet); margin-bottom: 7px; font-weight: 650;
}
.carte .valeur {
  font-size: 27px; font-weight: 700; font-variant-numeric: tabular-nums;
  letter-spacing: -.035em; line-height: 1.1;
}
.carte .note { font-size: 12.5px; color: var(--muet); margin-top: 5px; }
.carte.alerte { border-color: var(--rouge-trait); background: var(--rouge-clair); }
.carte.alerte .valeur { color: var(--rouge); }

.kpis { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: var(--e3); margin: 0 0 var(--e2); }
.kpi {
  position: relative; background: var(--surface);
  border: 1px solid var(--trait); border-radius: var(--rayon-lg);
  padding: 15px 16px; box-shadow: var(--ombre-1); overflow: hidden;
  transition: box-shadow var(--duree) var(--courbe), transform var(--duree) var(--courbe);
}
.kpi:hover { box-shadow: var(--ombre-2); }
.kpi::before {
  content: ""; position: absolute; inset: 0 auto 0 0; width: 3px;
  background: var(--trait-fort); opacity: .9;
}
.kpi.fort::before { background: linear-gradient(180deg, #6366f1, var(--accent)); }
.kpi.bon::before { background: var(--vert); }
.kpi.alerte::before { background: var(--rouge); }
.kpi .titre { font-size: 10.8px; text-transform: uppercase; letter-spacing: .07em; color: var(--muet); font-weight: 650; }
.kpi .valeur {
  font-size: 25px; font-weight: 700; font-variant-numeric: tabular-nums;
  letter-spacing: -.035em; margin-top: 6px; line-height: 1.12;
}
.kpi .note { font-size: 12px; color: var(--muet); margin-top: 4px; }
.kpi.fort .valeur { color: var(--accent); }
.kpi.bon .valeur { color: var(--vert); }
.kpi.alerte .valeur { color: var(--rouge); }

/* ===== boutons ========================================================= */
button, .bouton {
  font: inherit; font-size: 13.8px; font-weight: 600; line-height: 1.2;
  display: inline-flex; align-items: center; justify-content: center; gap: 6px;
  padding: 8px 14px; border-radius: var(--rayon);
  background: var(--accent); color: #fff; border: 1px solid transparent;
  cursor: pointer; white-space: nowrap;
  box-shadow: 0 1px 2px rgba(79, 70, 229, .25), inset 0 1px 0 rgba(255, 255, 255, .18);
  transition: background var(--duree) var(--courbe), box-shadow var(--duree), transform var(--duree);
}
button:hover, .bouton:hover { background: var(--accent-fonce); color: #fff; box-shadow: 0 3px 10px rgba(79, 70, 229, .3); }
button:active, .bouton:active { transform: translateY(1px); box-shadow: none; }
button:disabled, .bouton:disabled, .bouton.desactive { opacity: .45; cursor: not-allowed; transform: none; box-shadow: none; }
.bouton.secondaire, button.secondaire {
  background: var(--surface); color: var(--encre-douce); border: 1px solid var(--trait-fort);
  box-shadow: var(--ombre-1);
}
.bouton.secondaire:hover, button.secondaire:hover {
  background: var(--surface-3); color: var(--encre); border-color: var(--pale); box-shadow: var(--ombre-1);
}
.bouton.fantome, button.fantome { background: transparent; color: var(--muet); border-color: transparent; box-shadow: none; }
.bouton.fantome:hover, button.fantome:hover { background: var(--surface-3); color: var(--encre); box-shadow: none; }
.bouton.danger, button.danger { background: var(--surface); color: var(--rouge); border-color: var(--rouge-trait); box-shadow: var(--ombre-1); }
.bouton.danger:hover, button.danger:hover { background: var(--rouge-clair); color: var(--rouge); }
button.petit, .bouton.petit { padding: 5px 10px; font-size: 12.8px; border-radius: 6px; }
button svg, .bouton svg { width: 15px; height: 15px; margin-right: 1px; }

.actions { margin: var(--e4) 0; display: flex; gap: var(--e2); align-items: center; flex-wrap: wrap; }
.actions form { margin: 0; }

/* ===== champs ========================================================== */
input[type=text], input[type=number], input[type=email], input[type=date],
input[type=password], input[type=search], select, textarea {
  font: inherit; font-size: 13.8px; padding: 7px 10px;
  border: 1px solid var(--trait-fort); border-radius: var(--rayon);
  background: var(--surface); color: var(--encre); max-width: 100%;
  transition: border-color var(--duree), box-shadow var(--duree), background var(--duree);
}
input::placeholder, textarea::placeholder { color: var(--pale); }
input:hover, select:hover, textarea:hover { border-color: var(--pale); }
input:focus, select:focus, textarea:focus {
  outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--anneau); background: var(--surface);
}
input[type=checkbox], input[type=radio] { accent-color: var(--accent); width: 15px; height: 15px; }
textarea { resize: vertical; min-height: 76px; }
label { font-size: 13.5px; color: var(--encre-douce); font-weight: 500; }

.champ-court { width: 92px; }
.champ-moyen { width: 172px; }
.champ-large { min-width: 260px; }
.pleine { width: 100%; }

/* Un champ : libellé au-dessus, saisie pleine largeur. */
.champ { display: flex; flex-direction: column; gap: 5px; }
.champ > label { font-size: 12.5px; font-weight: 600; color: var(--encre-douce); }
.champ input, .champ select, .champ textarea { width: 100%; }

/* ===== pastilles et étiquettes ========================================= */
.pastille {
  display: inline-flex; align-items: center; gap: 6px;
  font-size: 11.5px; font-weight: 650; padding: 3px 9px;
  border-radius: 999px; border: 1px solid transparent; white-space: nowrap;
  letter-spacing: .01em;
}
.pastille::before { content: ""; width: 5px; height: 5px; border-radius: 50%; background: currentColor; }
.p-brouillon { background: var(--surface-3); color: var(--muet); border-color: var(--trait); }
.p-validee { background: var(--bleu-clair); color: var(--bleu); border-color: var(--bleu-trait); }
.p-payee { background: var(--vert-clair); color: var(--vert); border-color: var(--vert-trait); }
.p-retard { background: var(--rouge-clair); color: var(--rouge); border-color: var(--rouge-trait); }
.p-attente { background: var(--ambre-clair); color: var(--ambre); border-color: var(--ambre-trait); }

/* ===== messages ======================================================== */
.flash, .erreur, .alerte {
  display: flex; align-items: stretch; gap: 10px;
  padding: 11px 14px; margin: 0 0 var(--e4);
  border-radius: var(--rayon-lg); font-size: 13.8px; font-weight: 500;
  border: 1px solid transparent; box-shadow: var(--ombre-1);
}
.flash { background: var(--vert-clair); border-color: var(--vert-trait); color: var(--vert); }
.erreur { background: var(--rouge-clair); border-color: var(--rouge-trait); color: var(--rouge); }
.alerte.info { background: var(--bleu-clair); border-color: var(--bleu-trait); color: var(--bleu); }
.alerte.attention { background: var(--ambre-clair); border-color: var(--ambre-trait); color: var(--ambre); }
.flash::before, .erreur::before, .alerte::before {
  content: ""; width: 3px; border-radius: 2px; background: currentColor; opacity: .45; flex: 0 0 3px;
}

/* ===== aides =========================================================== */
.aide { color: var(--muet); font-size: 12.8px; }
.aide.petite { font-size: 12.2px; }
.aide.sous-champ { margin: 6px 0 0; }
.aide.bloc { margin: 0 0 var(--e3); }
.aide.haut { margin-top: 10px; }
.muet { color: var(--muet); }
.haut { margin-top: 10px; }
.rouge { color: var(--rouge); }
.en-ligne, form.en-ligne { display: inline; }
.petite { font-size: 12.5px; }
h1 .petite { font-weight: 400; color: var(--muet); letter-spacing: 0; }

/* ===== états vides ===================================================== */
.vide {
  display: flex; flex-direction: column; align-items: center; gap: 8px;
  color: var(--muet); padding: var(--e6) var(--e4); text-align: center;
  background: var(--surface); border: 1px dashed var(--trait-fort);
  border-radius: var(--rayon-lg); font-size: 13.8px;
}
.vide strong { color: var(--encre-douce); font-weight: 650; }
.vide a { font-weight: 600; }
.vide .geste { color: var(--pale); font-size: 12.8px; }

/* ===== fiches ========================================================== */
.fiche { display: grid; grid-template-columns: 2fr 1fr; gap: var(--e5); align-items: start; }
.fiche dl {
  margin: 0; background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 4px 18px; box-shadow: var(--ombre-1);
}
.fiche dt { font-size: 10.5px; text-transform: uppercase; letter-spacing: .07em; color: var(--pale); margin-top: 13px; font-weight: 650; }
.fiche dd { margin: 3px 0 13px; font-size: 14px; }
.total-fort { font-size: 22px; font-weight: 700; font-variant-numeric: tabular-nums; letter-spacing: -.03em; }

/* ===== tableau de bord : barres ======================================= */
.curve {
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 16px 18px 10px; box-shadow: var(--ombre-1);
}
.curve table { border: none; box-shadow: none; border-radius: 0; }
.curve th, .curve td { border: none; padding: 5px 6px; font-size: 12.5px; background: none; }
.curve thead th { position: static; text-transform: none; letter-spacing: 0; font-size: 11.5px; }
.curve .piste { background: var(--surface-3); height: 8px; width: 100%; border-radius: 999px; overflow: hidden; }
.curve .vente { background: linear-gradient(90deg, #6366f1, var(--accent)); height: 8px; border-radius: 999px; }
.curve .achat { background: var(--trait-fort); height: 8px; border-radius: 999px; }

/* ===== barre de filtres ================================================ */
.barre-outils {
  display: flex; gap: var(--e2); align-items: center; flex-wrap: wrap;
  padding: 10px var(--e3); background: var(--surface);
  border: 1px solid var(--trait); border-radius: var(--rayon-lg);
  margin: 0 0 var(--e4); box-shadow: var(--ombre-1);
}
.barre-outils form { display: flex; gap: var(--e2); align-items: center; flex-wrap: wrap; margin: 0; }
.barre-outils .pousse { margin-left: auto; }

/* ===== compte et pagination =========================================== */
.compte-liste { color: var(--muet); font-size: 12.8px; margin: 0; font-variant-numeric: tabular-nums; }
.compte-liste strong { color: var(--encre); font-weight: 700; font-size: 13.5px; }

/* ===== connexion ======================================================= */
.connexion { min-height: calc(100vh - 260px); display: grid; place-items: center; padding: var(--e5) 0; }
.connexion-carte {
  width: 100%; max-width: 384px;
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: 16px; padding: var(--e5) var(--e5) var(--e4);
  box-shadow: var(--ombre-3);
}
.connexion-marque { display: flex; align-items: center; gap: 9px; font-weight: 680; margin-bottom: var(--e4); font-size: 15px; }
.connexion-marque .sceau {
  display: grid; place-items: center; width: 30px; height: 30px;
  border-radius: 9px; color: #fff; font-size: 15px; font-weight: 700;
  background: linear-gradient(135deg, #6366f1 0%, #4f46e5 45%, #7c3aed 100%);
  box-shadow: 0 2px 8px rgba(79, 70, 229, .35);
}
.connexion h1 { font-size: 22px; margin-bottom: 3px; }
.connexion .sous-titre { margin-bottom: var(--e4); }
.connexion form { display: flex; flex-direction: column; gap: var(--e3); }

/* ===== adaptatif ======================================================= */
@media (max-width: 1080px) {
  .contenu { padding: var(--e4) var(--e4) 0; }
  header input[type=search] { width: 190px; }
  header input[type=search]:focus { width: 230px; }
}
@media (max-width: 900px) {
  header { padding: 0 12px; gap: var(--e2); flex-wrap: wrap; min-height: 0; }
  header nav { order: 3; width: 100%; }
  header nav a { padding: 6px 10px; }
  header nav a svg { display: none; }
  header form.recherche { margin-left: auto; }
  header input[type=search], header input[type=search]:focus { width: 150px; }
  header form.recherche kbd { display: none; }
  .onglets { padding: 0 12px; }
  .contenu { padding: var(--e3) 12px 0; }
  .fiche { grid-template-columns: 1fr; }
  .corps { display: block; }
  .menu-gauche {
    position: static; height: auto; width: auto; display: flex; overflow-x: auto;
    border-right: none; border-bottom: 1px solid var(--trait); padding: 8px;
  }
  .menu-gauche .module-titre, .menu-gauche hr { display: none; }
  .menu-gauche a, .menu-gauche span.avenir { white-space: nowrap; padding: 7px 10px; }
  .menu-gauche a svg { display: none; }
  thead th { position: static; }
  h1 { font-size: 22px; }
}
@media (max-width: 640px) {
  .cartes, .kpis { grid-template-columns: 1fr 1fr; }
  .carte .valeur, .kpi .valeur { font-size: 20px; }
}

/* ===== mode sombre : les mêmes jetons, remappés ======================== */
@media (prefers-color-scheme: dark) {
  :root {
    --encre: #f2f4f8;
    --encre-douce: #c8cede;
    --muet: #94a0b8;
    --pale: #6b7890;
    --papier: #0b0f19;
    --surface: #121826;
    --surface-2: #171d2d;
    --surface-3: #1d2436;
    --trait: #232b3d;
    --trait-fort: #33405a;
    --accent: #8b8cf7;
    --accent-fonce: #a5a6ff;
    --accent-clair: #1e2140;
    --accent-trait: #34376a;
    --anneau: rgba(139, 140, 247, .35);
    --vert: #4ade9b;
    --vert-clair: #0f2a22;
    --vert-trait: #1d4a3a;
    --rouge: #ff8b7e;
    --rouge-clair: #2c1512;
    --rouge-trait: #54241f;
    --bleu: #7cb0ff;
    --bleu-clair: #111d33;
    --bleu-trait: #243a5e;
    --ambre: #f5bf5c;
    --ambre-clair: #2a2110;
    --ambre-trait: #4a3a17;
    --ombre-1: 0 1px 2px rgba(0, 0, 0, .5);
    --ombre-2: 0 10px 30px rgba(0, 0, 0, .55);
    --ombre-3: 0 24px 48px rgba(0, 0, 0, .7);
  }
  header { background: color-mix(in srgb, var(--surface) 78%, transparent); }
  button, .bouton { color: #0b0f19; box-shadow: 0 1px 2px rgba(0, 0, 0, .5); }
  button:hover, .bouton:hover { color: #0b0f19; }
  .bouton.secondaire, button.secondaire, .bouton.fantome, button.fantome, .bouton.danger, button.danger { color: var(--encre-douce); }
  header .marque .sceau, .connexion-marque .sceau { color: #fff; }
}
"#;
