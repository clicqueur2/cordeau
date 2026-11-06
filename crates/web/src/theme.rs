//! Le système de style de l'interface, en un seul endroit.
//!
//! Trois idées, dans cet ordre.
//!
//! 1. **Un atelier, pas un tableau de bord SaaS.** Le fond est un papier chaud,
//!    l'encre est presque noire, l'accent est l'ocre du bâtiment. Un maçon et un
//!    architecte reconnaissent leur métier ; un bleu générique ne dit rien.
//!
//! 2. **Tout passe par des jetons.** Aucune couleur, aucune ombre, aucun rayon
//!    n'est écrit dans un écran. C'est ce qui permet le mode sombre sans
//!    reprendre une seule page, et c'est ce qui a manqué jusqu'ici : 220 styles
//!    en ligne échappaient à la palette.
//!
//! 3. **Dense, mais respirable.** Un logiciel de gestion affiche beaucoup de
//!    chiffres par écran : les nombres sont en chasse fixe, les lignes serrées,
//!    les titres courts. Le confort vient de l'alignement, pas du vide.
//!
//! Les noms de classes existants sont conservés (`aide`, `petit`, `secondaire`,
//! `champ-court`…), parce que soixante écrans les emploient. Ce fichier les
//! redessine, il ne les renomme pas.

/// La feuille de style complète, servie dans chaque page.
pub(crate) const CSS: &str = r#"
/* ===== jetons ========================================================== */
:root {
  /* Encres */
  --encre: #1b1a17;
  --encre-douce: #45403a;
  --muet: #6f675d;
  --pale: #9a9187;

  /* Surfaces */
  --papier: #f4f1ec;
  --surface: #ffffff;
  --surface-2: #faf8f4;
  --surface-3: #f1ede6;

  /* Traits */
  --trait: #e4dfd6;
  --trait-fort: #cfc6b8;

  /* Accent : l'ocre des chantiers */
  --accent: #9a5b2e;
  --accent-fonce: #7f4a24;
  --accent-clair: #f8efe6;
  --accent-trait: #e9d5c0;

  /* États */
  --vert: #2f6b45;
  --vert-clair: #e9f3ec;
  --vert-trait: #cbe0d3;
  --rouge: #a33a2a;
  --rouge-clair: #fcece9;
  --rouge-trait: #f0d3cc;
  --bleu: #2c5a78;
  --bleu-clair: #e9f1f6;
  --bleu-trait: #cfdfe9;
  --ambre: #87681c;
  --ambre-clair: #fbf4de;
  --ambre-trait: #ecdfb9;

  /* Formes */
  --rayon: 3px;
  --rayon-lg: 7px;
  --ombre-1: 0 1px 2px rgba(27, 26, 23, .05);
  --ombre-2: 0 4px 12px rgba(27, 26, 23, .07), 0 1px 2px rgba(27, 26, 23, .05);
  --duree: .14s;

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
  font: 15px/1.55 "Segoe UI", "Segoe UI Variable", system-ui, -apple-system, sans-serif;
  font-feature-settings: "kern" 1, "calt" 1;
  -webkit-font-smoothing: antialiased;
}
a { color: var(--accent); text-decoration: none; transition: color var(--duree) ease; }
a:hover { color: var(--accent-fonce); text-decoration: underline; text-underline-offset: 2px; }
:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 2px; }
::selection { background: var(--accent-trait); color: var(--encre); }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { transition-duration: 0ms !important; animation-duration: 0ms !important; scroll-behavior: auto !important; }
}

/* Lien d'évitement : premier élément focusable, invisible tant qu'on ne tabule pas. */
.saut {
  position: absolute; left: -9999px; top: 0; z-index: 100;
  background: var(--accent); color: #fff; padding: 10px 16px; border-radius: var(--rayon);
}
.saut:focus { left: 12px; top: 10px; }

/* ===== icônes ========================================================== */
.ic { width: 16px; height: 16px; vertical-align: -3px; margin-right: 7px; flex: 0 0 auto; }
.loupe { position: absolute; left: 9px; top: 50%; transform: translateY(-50%); display: flex; }
.loupe .ic { width: 15px; height: 15px; margin: 0; }

/* ===== connexion ======================================================= */
.connexion {
  min-height: calc(100vh - 220px);
  display: grid; place-items: center; padding: var(--e5) 0;
}
.connexion-carte {
  width: 100%; max-width: 372px;
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: 12px; padding: var(--e5) var(--e5) var(--e4);
  box-shadow: var(--ombre-2);
}
.connexion-marque {
  display: flex; align-items: center; gap: 9px;
  font-weight: 650; margin-bottom: var(--e4);
}
.connexion-marque .sceau {
  display: grid; place-items: center; width: 30px; height: 30px;
  border-radius: 7px; color: #fff; font-size: 15px; font-weight: 700;
  background: linear-gradient(150deg, var(--accent) 0%, var(--accent-fonce) 100%);
}
.connexion h1 { font-size: 21px; margin-bottom: 2px; }
.connexion .sous-titre { margin-bottom: var(--e4); }
.connexion form { display: flex; flex-direction: column; gap: var(--e3); }

/* Un champ : libellé au-dessus, saisie pleine largeur. */
.champ { display: flex; flex-direction: column; gap: 5px; }
.champ > label { font-size: 12.5px; font-weight: 600; color: var(--encre-douce); }
.champ input, .champ select, .champ textarea { width: 100%; }

/* ===== bandeau ========================================================= */
header {
  position: sticky; top: 0; z-index: 30;
  display: flex; align-items: center; gap: var(--e4);
  padding: 0 20px; min-height: 58px;
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  backdrop-filter: saturate(1.4) blur(8px);
  border-bottom: 1px solid var(--trait);
  box-shadow: var(--ombre-1);
}
header .marque {
  display: flex; align-items: center; gap: 10px;
  font-weight: 650; letter-spacing: -.01em; white-space: nowrap;
  margin-right: var(--e2); color: var(--encre); text-decoration: none;
}
header .marque:hover { text-decoration: none; color: var(--encre); }
header .marque .sceau {
  display: grid; place-items: center;
  width: 28px; height: 28px; border-radius: 6px;
  background: linear-gradient(150deg, var(--accent) 0%, var(--accent-fonce) 100%);
  color: #fff; font-size: 15px; font-weight: 700;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, .25), var(--ombre-1);
}
header .marque .suffixe { color: var(--pale); font-weight: 500; }
header nav { display: flex; gap: 2px; flex: 1; overflow-x: auto; scrollbar-width: none; }
header nav::-webkit-scrollbar { display: none; }
header nav a {
  color: var(--encre-douce); padding: 18px 12px 16px;
  border-bottom: 2px solid transparent; white-space: nowrap;
  font-size: 14px; border-radius: 4px 4px 0 0;
  display: inline-flex; align-items: center; gap: 7px;
}
header nav a svg { width: 16px; height: 16px; opacity: .75; }
header nav a:hover { background: var(--surface-2); text-decoration: none; color: var(--encre); }
header nav a.actif {
  border-bottom-color: var(--accent); color: var(--accent); font-weight: 600;
}
header nav a.actif svg { opacity: 1; }
header nav a.inactif, header nav span.inactif {
  color: var(--pale); padding: 18px 12px 16px; border-bottom: 2px solid transparent;
  white-space: nowrap; font-size: 14px;
}

/* Recherche : icône dans le champ, raccourci affiché à droite. */
header form.recherche { display: flex; align-items: center; margin: 0; position: relative; }
header form.recherche .loupe {
  position: absolute; left: 9px; width: 15px; height: 15px; color: var(--pale); pointer-events: none;
}
header input[type=search] {
  font: inherit; font-size: 13.5px;
  padding: 8px 12px 8px 30px; width: 230px;
  border: 1px solid var(--trait); border-radius: var(--rayon);
  background: var(--surface-2); color: var(--encre);
  transition: border-color var(--duree) ease, background var(--duree) ease, width var(--duree) ease;
}
header input[type=search]:focus {
  background: var(--surface); border-color: var(--accent-trait); width: 290px; outline: none;
}
header form.recherche button { display: none; }
header form.recherche kbd {
  position: absolute; right: 8px; font: 600 11px/1 ui-monospace, monospace;
  color: var(--pale); background: var(--surface-3); border: 1px solid var(--trait);
  border-radius: 3px; padding: 2px 5px;
}

/* Sélecteur de société : c'est un filtre, il doit se voir comme tel. */
header form.societe { display: flex; align-items: center; gap: 6px; margin: 0; }
header form.societe select {
  font: inherit; font-size: 13px; padding: 7px 8px;
  border: 1px solid var(--trait); border-radius: var(--rayon);
  background: var(--surface); color: var(--encre); max-width: 210px;
}
header form.societe .mention { color: var(--pale); font-size: 11.5px; white-space: nowrap; }
header .compte { display: flex; align-items: center; gap: 8px; margin: 0; }
header .compte form { margin: 0; }

/* ===== sous-onglets ==================================================== */
.onglets {
  display: flex; gap: 4px; padding: 0 32px;
  background: var(--surface); border-bottom: 1px solid var(--trait);
  overflow-x: auto; scrollbar-width: none;
}
.onglets::-webkit-scrollbar { display: none; }
.onglets a {
  padding: 10px 12px; font-size: 13.5px; color: var(--muet);
  border-bottom: 2px solid transparent; white-space: nowrap;
}
.onglets a:hover { color: var(--encre); text-decoration: none; background: var(--surface-2); }
.onglets a.actif { color: var(--accent); border-bottom-color: var(--accent); font-weight: 600; }

/* ===== corps =========================================================== */
.corps { display: flex; align-items: flex-start; }
.menu-gauche {
  width: 232px; flex: 0 0 232px;
  position: sticky; top: 58px; align-self: flex-start;
  height: calc(100vh - 58px); overflow-y: auto;
  background: var(--surface); border-right: 1px solid var(--trait);
  padding: var(--e3) 0 var(--e6);
}
.menu-gauche .module-titre {
  display: flex; align-items: center; gap: 7px;
  font-size: 10.5px; text-transform: uppercase; letter-spacing: .1em;
  color: var(--pale); padding: 10px 16px 7px; font-weight: 700;
}
.menu-gauche .module-titre svg { width: 14px; height: 14px; }
.menu-gauche a, .menu-gauche span.avenir {
  display: flex; align-items: center; gap: 8px;
  padding: 7px 16px; font-size: 13.8px; color: var(--encre-douce);
  border-left: 2px solid transparent; margin: 1px 0;
  transition: background var(--duree) ease, color var(--duree) ease;
}
.menu-gauche a svg, .menu-gauche span.avenir svg { width: 15px; height: 15px; opacity: .6; flex: 0 0 15px; }
.menu-gauche a:hover { background: var(--surface-2); text-decoration: none; color: var(--encre); }
.menu-gauche a.actif {
  background: var(--accent-clair); border-left-color: var(--accent);
  color: var(--accent-fonce); font-weight: 600;
}
.menu-gauche a.actif svg { opacity: 1; }
.menu-gauche span.avenir { color: var(--pale); }
.menu-gauche span.avenir em { font-style: normal; font-size: 11px; color: var(--pale); margin-left: auto; }
.menu-gauche hr { border: none; border-top: 1px solid var(--trait); margin: var(--e3) 12px; }

main { flex: 1; min-width: 0; padding: 0 0 80px; }
.contenu { padding: var(--e5) var(--e6) 0; max-width: 1120px; }

/* ===== titres et texte ================================================= */
h1 {
  font-size: 25px; font-weight: 650; letter-spacing: -.02em;
  margin: 0 0 4px; line-height: 1.2;
}
.sous-titre { color: var(--muet); margin: 0 0 var(--e5); font-size: 14px; }
.sous-titre strong { color: var(--encre-douce); font-weight: 600; }
h2 {
  font-size: 11.5px; text-transform: uppercase; letter-spacing: .1em;
  color: var(--pale); margin: var(--e6) 0 var(--e3); font-weight: 700;
}
h3 { font-size: 15.5px; font-weight: 650; margin: var(--e5) 0 var(--e2); }
p { margin: 0 0 var(--e3); }

/* ===== tableaux ======================================================== */
table {
  width: 100%; border-collapse: separate; border-spacing: 0;
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); overflow: hidden; box-shadow: var(--ombre-1);
}
thead th {
  position: sticky; top: 58px; z-index: 2;
  text-align: left; font-weight: 600; font-size: 11.5px;
  text-transform: uppercase; letter-spacing: .06em; color: var(--muet);
  padding: 10px 14px; border-bottom: 1px solid var(--trait);
  background: var(--surface-2); white-space: nowrap;
}
td { padding: 10px 14px; border-bottom: 1px solid var(--trait); vertical-align: top; }
tbody tr:nth-child(even) { background: color-mix(in srgb, var(--surface-2) 55%, var(--surface)); }
tbody tr { transition: background var(--duree) ease; }
tbody tr:hover { background: var(--accent-clair); }
tbody tr:last-child td { border-bottom: none; }
tfoot td {
  border-top: 1px solid var(--trait-fort); background: var(--surface-2);
  font-weight: 650;
}
.num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
.ref { font-variant-numeric: tabular-nums; letter-spacing: .01em; font-size: 13.5px; }
td .aide { display: block; margin-top: 2px; }

/* ===== cartes et indicateurs =========================================== */
.cartes { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: var(--e3); }
.carte {
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 16px 18px; box-shadow: var(--ombre-1);
}
.carte .libelle {
  font-size: 11px; text-transform: uppercase; letter-spacing: .09em;
  color: var(--pale); margin-bottom: 6px; font-weight: 600;
}
.carte .valeur {
  font-size: 27px; font-weight: 650; font-variant-numeric: tabular-nums;
  letter-spacing: -.03em; line-height: 1.1;
}
.carte .note { font-size: 12.5px; color: var(--muet); margin-top: 4px; }
.carte.alerte { border-color: var(--rouge-trait); background: var(--rouge-clair); }
.carte.alerte .valeur { color: var(--rouge); }

.kpis {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(158px, 1fr));
  gap: 1px; background: var(--trait);
  border: 1px solid var(--trait); border-radius: var(--rayon-lg);
  overflow: hidden; margin: 0 0 var(--e2); box-shadow: var(--ombre-1);
}
.kpi { background: var(--surface); padding: 14px 16px; position: relative; }
.kpi::after {
  content: ""; position: absolute; left: 0; top: 0; bottom: 0; width: 3px;
  background: transparent; transition: background var(--duree) ease;
}
.kpi:hover::after { background: var(--accent-trait); }
.kpi .titre { font-size: 10.5px; text-transform: uppercase; letter-spacing: .1em; color: var(--pale); font-weight: 600; }
.kpi .valeur {
  font-size: 24px; font-weight: 650; font-variant-numeric: tabular-nums;
  letter-spacing: -.025em; margin-top: 5px; line-height: 1.15;
}
.kpi .note { font-size: 12px; color: var(--muet); margin-top: 3px; }
.kpi.fort .valeur { color: var(--accent); }
.kpi.fort::after { background: var(--accent); }
.kpi.bon .valeur { color: var(--vert); }
.kpi.bon::after { background: var(--vert); }
.kpi.alerte .valeur { color: var(--rouge); }
.kpi.alerte::after { background: var(--rouge); }

/* ===== boutons ========================================================= */
button, .bouton {
  font: inherit; font-size: 14px; font-weight: 550;
  display: inline-flex; align-items: center; justify-content: center; gap: 7px;
  padding: 8px 15px; border-radius: var(--rayon);
  background: var(--accent); color: #fff; border: 1px solid var(--accent-fonce);
  cursor: pointer; white-space: nowrap;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, .12), var(--ombre-1);
  transition: background var(--duree) ease, transform var(--duree) ease, box-shadow var(--duree) ease;
}
button:hover, .bouton:hover { background: var(--accent-fonce); color: #fff; text-decoration: none; }
button:active, .bouton:active { transform: translateY(1px); box-shadow: none; }
button:disabled, .bouton:disabled, .bouton.desactive { opacity: .5; cursor: not-allowed; transform: none; }
.bouton.secondaire, button.secondaire {
  background: var(--surface); color: var(--encre-douce); border: 1px solid var(--trait-fort);
  box-shadow: var(--ombre-1);
}
.bouton.secondaire:hover, button.secondaire:hover {
  background: var(--surface-2); color: var(--encre); border-color: var(--pale);
}
.bouton.fantome, button.fantome {
  background: transparent; color: var(--muet); border-color: transparent; box-shadow: none;
}
.bouton.fantome:hover, button.fantome:hover { background: var(--surface-3); color: var(--encre); }
.bouton.danger, button.danger {
  background: var(--surface); color: var(--rouge); border-color: var(--rouge-trait);
}
.bouton.danger:hover, button.danger:hover { background: var(--rouge-clair); color: var(--rouge); }
button.petit, .bouton.petit { padding: 5px 10px; font-size: 13px; border-radius: var(--rayon); }
button svg, .bouton svg { width: 15px; height: 15px; }

.actions {
  margin: var(--e4) 0; display: flex; gap: var(--e2);
  align-items: center; flex-wrap: wrap;
}
.actions form { margin: 0; }

/* ===== champs ========================================================== */
input[type=text], input[type=number], input[type=email], input[type=date],
input[type=password], input[type=search], select, textarea {
  font: inherit; font-size: 14px; padding: 7px 9px;
  border: 1px solid var(--trait-fort); border-radius: var(--rayon);
  background: var(--surface); color: var(--encre);
  transition: border-color var(--duree) ease, box-shadow var(--duree) ease;
  max-width: 100%;
}
input:hover, select:hover, textarea:hover { border-color: var(--pale); }
input:focus, select:focus, textarea:focus {
  outline: none; border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-clair);
}
input[type=checkbox], input[type=radio] { accent-color: var(--accent); width: 15px; height: 15px; }
textarea { resize: vertical; min-height: 74px; }
label { font-size: 13.5px; color: var(--encre-douce); }

/* Trois largeurs suffisent : court (quantité), moyen (date, montant), large (liste). */
.champ-court { width: 92px; }
.champ-moyen { width: 172px; }
.champ-large { min-width: 260px; }
.pleine { width: 100%; }

/* ===== pastilles et étiquettes ========================================= */
.pastille {
  display: inline-flex; align-items: center; gap: 5px;
  font-size: 11.5px; font-weight: 600; padding: 2px 9px;
  border-radius: 999px; border: 1px solid transparent; white-space: nowrap;
  letter-spacing: .01em;
}
.pastille::before {
  content: ""; width: 5px; height: 5px; border-radius: 50%; background: currentColor; opacity: .75;
}
.p-brouillon { background: var(--surface-3); color: var(--muet); border-color: var(--trait); }
.p-validee { background: var(--bleu-clair); color: var(--bleu); border-color: var(--bleu-trait); }
.p-payee { background: var(--vert-clair); color: var(--vert); border-color: var(--vert-trait); }
.p-retard { background: var(--rouge-clair); color: var(--rouge); border-color: var(--rouge-trait); }
.p-attente { background: var(--ambre-clair); color: var(--ambre); border-color: var(--ambre-trait); }

/* ===== messages ======================================================== */
.flash, .erreur, .alerte {
  display: flex; align-items: flex-start; gap: 9px;
  padding: 11px 14px; margin: 0 0 var(--e5);
  border-radius: var(--rayon-lg); font-size: 14px;
  border: 1px solid transparent;
}
.flash { background: var(--vert-clair); border-color: var(--vert-trait); color: var(--vert); }
.erreur { background: var(--rouge-clair); border-color: var(--rouge-trait); color: var(--rouge); }
.alerte.info { background: var(--bleu-clair); border-color: var(--bleu-trait); color: var(--bleu); }
.alerte.attention { background: var(--ambre-clair); border-color: var(--ambre-trait); color: var(--ambre); }
.flash::before, .erreur::before, .alerte::before {
  content: ""; width: 3px; align-self: stretch; border-radius: 2px; background: currentColor; opacity: .5;
}

/* ===== aides (78 usages) =============================================== */
.aide { color: var(--muet); font-size: 13px; }
.aide.petite { font-size: 12.5px; }
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
  border-radius: var(--rayon-lg);
}
.vide strong { color: var(--encre-douce); font-weight: 600; }
.vide a { font-weight: 600; }
.vide .geste { color: var(--pale); font-size: 13px; }

/* ===== fiches ========================================================== */
.fiche { display: grid; grid-template-columns: 2fr 1fr; gap: var(--e5); align-items: start; }
.fiche dl {
  margin: 0; background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 4px 18px; box-shadow: var(--ombre-1);
}
.fiche dt {
  font-size: 10.5px; text-transform: uppercase; letter-spacing: .08em;
  color: var(--pale); margin-top: 12px; font-weight: 600;
}
.fiche dd { margin: 3px 0 12px; }
.total-fort { font-size: 22px; font-weight: 650; font-variant-numeric: tabular-nums; letter-spacing: -.02em; }

/* ===== tableau de bord : barres ======================================= */
.curve {
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 16px 18px 10px; box-shadow: var(--ombre-1);
}
.curve table { border: none; box-shadow: none; border-radius: 0; }
.curve th, .curve td { border: none; padding: 4px 6px; font-size: 12.5px; background: none; }
.curve thead th { position: static; text-transform: none; letter-spacing: 0; }
.curve .piste { background: var(--surface-3); height: 9px; width: 100%; border-radius: 999px; overflow: hidden; }
.curve .vente { background: var(--accent); height: 9px; border-radius: 999px; }
.curve .achat { background: var(--trait-fort); height: 9px; border-radius: 999px; }

/* ===== barre de filtres ================================================ */
.barre-outils {
  display: flex; gap: var(--e2); align-items: center; flex-wrap: wrap;
  padding: var(--e3); background: var(--surface); border: 1px solid var(--trait);
  border-radius: var(--rayon-lg); margin: 0 0 var(--e4); box-shadow: var(--ombre-1);
}
.barre-outils form { display: flex; gap: var(--e2); align-items: center; flex-wrap: wrap; margin: 0; }
.barre-outils .pousse { margin-left: auto; }

/* ===== compte et pagination =========================================== */
.compte-liste { color: var(--muet); font-size: 13px; margin: 0 0 var(--e3); }
.compte-liste strong { color: var(--encre-douce); font-weight: 650; font-variant-numeric: tabular-nums; }

/* ===== adaptatif ======================================================= */
@media (max-width: 1080px) {
  .contenu { padding: var(--e4) var(--e4) 0; }
  header input[type=search] { width: 180px; }
  header input[type=search]:focus { width: 220px; }
}
@media (max-width: 900px) {
  header { padding: 0 12px; gap: var(--e2); flex-wrap: wrap; min-height: 0; }
  header nav { order: 3; width: 100%; }
  header nav a { padding: 10px 10px 8px; }
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
    border-right: none; border-bottom: 1px solid var(--trait); padding: 0 8px;
  }
  .menu-gauche .module-titre, .menu-gauche hr { display: none; }
  .menu-gauche a, .menu-gauche span.avenir {
    white-space: nowrap; border-left: none; border-bottom: 2px solid transparent; padding: 10px;
  }
  .menu-gauche a svg { display: none; }
  .menu-gauche a.actif { border-bottom-color: var(--accent); }
  thead th { position: static; }
  h1 { font-size: 21px; }
}
@media (max-width: 640px) {
  .cartes { grid-template-columns: 1fr 1fr; }
  .kpis { grid-template-columns: 1fr 1fr; }
  .carte .valeur, .kpi .valeur { font-size: 20px; }
}

/* ===== mode sombre : les mêmes jetons, remappés ======================== */
@media (prefers-color-scheme: dark) {
  :root {
    --encre: #ede9e2;
    --encre-douce: #cfc8bd;
    --muet: #a49b8f;
    --pale: #857c70;
    --papier: #16150f;
    --surface: #1e1c17;
    --surface-2: #24221b;
    --surface-3: #2b2820;
    --trait: #33302a;
    --trait-fort: #45413a;
    --accent: #d79a63;
    --accent-fonce: #e5ad78;
    --accent-clair: #2c2518;
    --accent-trait: #4c3a22;
    --vert: #7fbf98;
    --vert-clair: #1a2a1f;
    --vert-trait: #2f4a37;
    --rouge: #e08c7c;
    --rouge-clair: #2e1c18;
    --rouge-trait: #513029;
    --bleu: #85b6d4;
    --bleu-clair: #16232b;
    --bleu-trait: #2c4454;
    --ambre: #d8b464;
    --ambre-clair: #2a2313;
    --ambre-trait: #4a3d1d;
    --ombre-1: 0 1px 2px rgba(0, 0, 0, .4);
    --ombre-2: 0 6px 18px rgba(0, 0, 0, .5);
  }
  header { background: color-mix(in srgb, var(--surface) 88%, transparent); }
  button, .bouton { box-shadow: var(--ombre-1); }
  header .marque .sceau { box-shadow: inset 0 1px 0 rgba(255, 255, 255, .12); }
}
"#;
