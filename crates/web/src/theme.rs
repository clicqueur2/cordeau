//! Le système de style de l'interface, en un seul endroit.
//!
//! Les conventions suivies sont celles d'un système d'exploitation, pas celles
//! d'un site : gris système, séparateurs d'un demi-pixel, verre dépoli, contrôles
//! de 30 px, rayons de 8 à 12, une seule teinte d'action. Ce qui fait « propre »
//! n'est pas une couleur, c'est la constance des mesures.
//!
//! Tout passe par des jetons : le mode sombre bascule par remappage, sans
//! reprendre une seule page. Les noms de classes existants (`aide`, `petit`,
//! `secondaire`, `champ-court`…) sont conservés parce que soixante écrans les
//! emploient : ce fichier les redessine, il ne les renomme pas.

/// La feuille de style complète, servie dans chaque page.
pub(crate) const CSS: &str = r#"
/* ===== jetons ========================================================== */
:root {
  --encre: #1d1d1f;
  --encre-douce: #3a3a3c;
  --muet: #6e6e73;
  --pale: #8e8e93;

  --papier: #f2f2f7;
  --surface: #ffffff;
  --surface-2: #fbfbfd;
  --surface-3: #f5f5f7;
  --verre: rgba(250, 250, 252, .72);

  --trait: rgba(0, 0, 0, .07);
  --trait-fort: rgba(0, 0, 0, .14);

  --accent: #0071e3;
  --accent-fonce: #0062c4;
  --accent-clair: rgba(0, 113, 227, .1);
  --accent-trait: rgba(0, 113, 227, .28);
  --anneau: rgba(0, 113, 227, .35);

  --vert: #1d7a4d;
  --vert-clair: rgba(29, 122, 77, .1);
  --vert-trait: rgba(29, 122, 77, .24);
  --rouge: #c0392b;
  --rouge-clair: rgba(192, 57, 43, .09);
  --rouge-trait: rgba(192, 57, 43, .24);
  --bleu: #0062c4;
  --bleu-clair: rgba(0, 113, 227, .09);
  --bleu-trait: rgba(0, 113, 227, .22);
  --ambre: #9a6508;
  --ambre-clair: rgba(154, 101, 8, .1);
  --ambre-trait: rgba(154, 101, 8, .24);

  --rayon: 8px;
  --rayon-lg: 12px;
  --ombre-1: 0 0.5px 1px rgba(0, 0, 0, .06), 0 0 0 0.5px rgba(0, 0, 0, .04);
  --ombre-2: 0 4px 14px rgba(0, 0, 0, .07), 0 0 0 0.5px rgba(0, 0, 0, .05);
  --ombre-3: 0 18px 40px rgba(0, 0, 0, .14), 0 0 0 0.5px rgba(0, 0, 0, .06);
  --duree: .18s;
  --courbe: cubic-bezier(.32, .72, 0, 1);

  --e1: 4px; --e2: 8px; --e3: 12px; --e4: 16px; --e5: 24px; --e6: 32px;
}

/* ===== base ============================================================ */
*, *::before, *::after { box-sizing: border-box; }
html { -webkit-text-size-adjust: 100%; scroll-behavior: smooth; }
body {
  margin: 0;
  color: var(--encre);
  background: var(--papier);
  font: 14px/1.5 -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
  font-feature-settings: "kern" 1, "calt" 1;
  -webkit-font-smoothing: antialiased;
}
a { color: var(--accent); text-decoration: none; transition: color var(--duree) var(--courbe); }
a:hover { color: var(--accent-fonce); }
:focus-visible { outline: none; box-shadow: 0 0 0 3.5px var(--anneau); border-radius: 6px; }
::selection { background: rgba(0, 113, 227, .22); }
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
.loupe { position: absolute; left: 10px; top: 50%; transform: translateY(-50%); display: flex; color: var(--pale); }
.loupe .ic { width: 14px; height: 14px; margin: 0; }

/* ===== bandeau : verre dépoli, comme une barre de titre ================ */
header {
  position: sticky; top: 0; z-index: 30;
  display: flex; align-items: center; gap: var(--e3);
  padding: 0 16px; min-height: 52px;
  background: var(--verre);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  border-bottom: 0.5px solid var(--trait-fort);
}
header .marque {
  display: flex; align-items: center; gap: 8px;
  font-weight: 600; letter-spacing: -.015em; white-space: nowrap;
  font-size: 14.5px; color: var(--encre); margin-right: var(--e2);
}
header .marque .sceau {
  display: grid; place-items: center; width: 26px; height: 26px;
  border-radius: 7px; color: #fff; font-size: 13px; font-weight: 600;
  background: linear-gradient(160deg, #4da3ff 0%, #0071e3 55%, #0058b8 100%);
  box-shadow: inset 0 0.5px 0 rgba(255, 255, 255, .5), 0 1px 2px rgba(0, 0, 0, .18);
}
header .marque .suffixe { color: var(--pale); font-weight: 400; }

header nav { display: flex; gap: 1px; flex: 1; overflow-x: auto; scrollbar-width: none; }
header nav::-webkit-scrollbar { display: none; }
header nav a {
  display: inline-flex; align-items: center; gap: 6px;
  color: var(--encre-douce); padding: 5px 10px; border-radius: 7px;
  white-space: nowrap; font-size: 13px; font-weight: 400;
  transition: background var(--duree) var(--courbe), color var(--duree) var(--courbe);
}
header nav a svg { width: 15px; height: 15px; opacity: .6; }
header nav a:hover { background: rgba(0, 0, 0, .05); color: var(--encre); }
header nav a.actif { background: var(--accent-clair); color: var(--accent); font-weight: 500; }
header nav a.actif svg { opacity: 1; }
header nav a.inactif, header nav span.inactif {
  color: var(--pale); padding: 5px 10px; white-space: nowrap; font-size: 13px;
}

header form.recherche { display: flex; align-items: center; margin: 0; position: relative; }
header input[type=search] {
  font: inherit; font-size: 13px; color: var(--encre);
  padding: 0 10px 0 30px; width: 220px; height: 30px;
  border: 0.5px solid var(--trait-fort); border-radius: 8px;
  background: var(--surface);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, .04);
  transition: border-color var(--duree), box-shadow var(--duree), width var(--duree);
}
header input[type=search]:focus {
  border-color: var(--accent); box-shadow: 0 0 0 3.5px var(--anneau); width: 280px; outline: none;
}
header input[type=search]::placeholder { color: var(--pale); }
header form.recherche button { display: none; }
header form.recherche kbd {
  position: absolute; right: 8px; font: 400 10.5px/1 -apple-system, "Segoe UI", sans-serif;
  color: var(--pale); background: var(--surface-3); border: 0.5px solid var(--trait-fort);
  border-radius: 4px; padding: 2px 4px; pointer-events: none;
}

header form.societe { display: flex; align-items: center; gap: 6px; margin: 0; }
header form.societe select {
  font: inherit; font-size: 12.5px; height: 30px; padding: 0 8px;
  border: 0.5px solid var(--trait-fort); border-radius: 8px;
  background: var(--surface); color: var(--encre);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, .04); max-width: 200px;
}
header form.societe .mention { color: var(--pale); font-size: 10.5px; white-space: nowrap; }
header .compte { display: flex; align-items: center; gap: 4px; margin: 0; }
header .compte form { margin: 0; }

/* ===== sous-onglets ==================================================== */
.onglets {
  display: flex; gap: 2px; padding: 0 var(--e6);
  background: var(--verre); border-bottom: 0.5px solid var(--trait);
  overflow-x: auto; scrollbar-width: none;
}
.onglets::-webkit-scrollbar { display: none; }
.onglets a { padding: 9px 10px; font-size: 12.8px; color: var(--muet); border-bottom: 2px solid transparent; white-space: nowrap; }
.onglets a:hover { color: var(--encre); }
.onglets a.actif { color: var(--accent); border-bottom-color: var(--accent); font-weight: 500; }

/* ===== corps =========================================================== */
.corps { display: flex; align-items: flex-start; }
.menu-gauche {
  width: 228px; flex: 0 0 228px;
  position: sticky; top: 52px; align-self: flex-start;
  height: calc(100vh - 52px); overflow-y: auto;
  background: rgba(246, 246, 249, .8);
  backdrop-filter: blur(20px);
  border-right: 0.5px solid var(--trait);
  padding: var(--e3) 8px var(--e6);
}
.menu-gauche .module-titre {
  display: flex; align-items: center; gap: 6px;
  font-size: 10.5px; letter-spacing: .01em;
  color: var(--pale); padding: 9px 8px 5px; font-weight: 600;
}
.menu-gauche .module-titre svg { width: 13px; height: 13px; margin-right: 1px; }
.menu-gauche a, .menu-gauche span.avenir {
  display: flex; align-items: center; gap: 7px;
  padding: 5.5px 8px; font-size: 13px; color: var(--encre-douce);
  border-radius: 7px; margin: 0.5px 0; font-weight: 400;
  transition: background var(--duree) var(--courbe);
}
.menu-gauche a svg, .menu-gauche span.avenir svg { width: 14px; height: 14px; opacity: .5; flex: 0 0 14px; margin-right: 1px; }
.menu-gauche a:hover { background: rgba(0, 0, 0, .05); color: var(--encre); }
.menu-gauche a.actif { background: var(--accent-clair); color: var(--accent); font-weight: 500; }
.menu-gauche a.actif svg { opacity: 1; }
.menu-gauche span.avenir { color: var(--pale); }
.menu-gauche span.avenir em { font-style: normal; font-size: 10px; color: var(--pale); margin-left: auto; }
.menu-gauche hr { border: none; border-top: 0.5px solid var(--trait); margin: var(--e3) 6px; }

main { flex: 1; min-width: 0; padding: 0 0 80px; }
.contenu { padding: var(--e5) var(--e6) 0; max-width: 1160px; }

/* ===== titres et texte ================================================= */
h1 { font-size: 28px; font-weight: 700; letter-spacing: -.035em; margin: 0 0 4px; line-height: 1.14; }
.sous-titre { color: var(--muet); margin: 0 0 var(--e5); font-size: 13.5px; }
.sous-titre strong { color: var(--encre-douce); font-weight: 600; }
h2 { font-size: 12.5px; letter-spacing: -.01em; color: var(--encre-douce); margin: var(--e6) 0 var(--e3); font-weight: 600; }
h3 { font-size: 15px; font-weight: 600; letter-spacing: -.015em; margin: var(--e5) 0 var(--e2); }
p { margin: 0 0 var(--e3); }

/* ===== tableaux ======================================================== */
table {
  width: 100%; border-collapse: separate; border-spacing: 0;
  background: var(--surface); border: 0.5px solid var(--trait);
  border-radius: var(--rayon-lg); overflow: hidden;
  box-shadow: var(--ombre-1); font-size: 13.5px;
}
thead th {
  position: sticky; top: 52px; z-index: 2;
  text-align: left; font-weight: 500; font-size: 11px;
  letter-spacing: .01em; color: var(--muet);
  padding: 9px 14px; border-bottom: 0.5px solid var(--trait-fort);
  background: var(--surface-2); white-space: nowrap;
}
td { padding: 10px 14px; border-bottom: 0.5px solid var(--trait); vertical-align: middle; }
tbody tr { transition: background var(--duree) var(--courbe); }
tbody tr:hover { background: var(--surface-3); }
tbody tr:last-child td { border-bottom: none; }
tfoot td { border-top: 0.5px solid var(--trait-fort); background: var(--surface-2); font-weight: 600; }
.num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
.ref { font-variant-numeric: tabular-nums; letter-spacing: -.01em; font-size: 13px; font-weight: 500; }
td .aide { display: block; margin-top: 2px; }

/* ===== cartes et indicateurs =========================================== */
.cartes { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: var(--e3); }
.carte {
  background: var(--surface); border: 0.5px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 15px 17px; box-shadow: var(--ombre-1);
  transition: box-shadow var(--duree) var(--courbe);
}
.carte:hover { box-shadow: var(--ombre-2); }
.carte .libelle { font-size: 11px; letter-spacing: .01em; color: var(--muet); margin-bottom: 6px; font-weight: 500; }
.carte .valeur { font-size: 26px; font-weight: 600; font-variant-numeric: tabular-nums; letter-spacing: -.035em; line-height: 1.1; }
.carte .note { font-size: 12px; color: var(--muet); margin-top: 4px; }
.carte.alerte { border-color: var(--rouge-trait); background: var(--rouge-clair); }
.carte.alerte .valeur { color: var(--rouge); }

.kpis { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: var(--e3); margin: 0 0 var(--e2); }
.kpi {
  position: relative; background: var(--surface); border: 0.5px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 14px 16px; box-shadow: var(--ombre-1);
  overflow: hidden; transition: box-shadow var(--duree) var(--courbe);
}
.kpi:hover { box-shadow: var(--ombre-2); }
.kpi::before { content: ""; position: absolute; inset: 0 auto 0 0; width: 3px; background: var(--trait-fort); }
.kpi.fort::before { background: var(--accent); }
.kpi.bon::before { background: var(--vert); }
.kpi.alerte::before { background: var(--rouge); }
.kpi .titre { font-size: 11px; color: var(--muet); font-weight: 500; }
.kpi .valeur { font-size: 24px; font-weight: 600; font-variant-numeric: tabular-nums; letter-spacing: -.035em; margin-top: 5px; line-height: 1.12; }
.kpi .note { font-size: 11.5px; color: var(--muet); margin-top: 3px; }
.kpi.fort .valeur { color: var(--accent); }
.kpi.bon .valeur { color: var(--vert); }
.kpi.alerte .valeur { color: var(--rouge); }

/* ===== boutons ========================================================= */
button, .bouton {
  font: inherit; font-size: 13px; font-weight: 500; line-height: 1;
  display: inline-flex; align-items: center; justify-content: center; gap: 5px;
  padding: 0 13px; height: 30px; border-radius: var(--rayon);
  background: var(--accent); color: #fff; border: 0.5px solid transparent;
  cursor: pointer; white-space: nowrap;
  box-shadow: 0 0.5px 1.5px rgba(0, 0, 0, .16), inset 0 0.5px 0 rgba(255, 255, 255, .28);
  transition: background var(--duree) var(--courbe), transform var(--duree);
}
button:hover, .bouton:hover { background: var(--accent-fonce); color: #fff; }
button:active, .bouton:active { transform: scale(.985); }
button:disabled, .bouton:disabled, .bouton.desactive { opacity: .4; cursor: not-allowed; transform: none; }
.bouton.secondaire, button.secondaire {
  background: var(--surface); color: var(--encre-douce); border: 0.5px solid var(--trait-fort);
  box-shadow: 0 0.5px 1px rgba(0, 0, 0, .06);
}
.bouton.secondaire:hover, button.secondaire:hover { background: var(--surface-3); color: var(--encre); }
.bouton.fantome, button.fantome { background: transparent; border-color: transparent; color: var(--accent); box-shadow: none; }
.bouton.fantome:hover, button.fantome:hover { background: rgba(0, 0, 0, .05); color: var(--accent-fonce); }
.bouton.danger, button.danger { background: var(--surface); color: var(--rouge); border-color: var(--rouge-trait); box-shadow: 0 0.5px 1px rgba(0, 0, 0, .06); }
.bouton.danger:hover, button.danger:hover { background: var(--rouge-clair); color: var(--rouge); }
button.petit, .bouton.petit { height: 25px; padding: 0 9px; font-size: 12px; border-radius: 6px; }
button svg, .bouton svg { width: 14px; height: 14px; margin-right: 1px; }

.actions { margin: var(--e4) 0; display: flex; gap: var(--e2); align-items: center; flex-wrap: wrap; }
.actions form { margin: 0; }

/* ===== champs ========================================================== */
input[type=text], input[type=number], input[type=email], input[type=date],
input[type=password], input[type=search], select, textarea {
  font: inherit; font-size: 13px; color: var(--encre);
  padding: 0 9px; height: 30px;
  border: 0.5px solid var(--trait-fort); border-radius: var(--rayon);
  background: var(--surface); max-width: 100%;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, .04);
  transition: border-color var(--duree), box-shadow var(--duree);
}
textarea { height: auto; padding: 7px 9px; min-height: 76px; resize: vertical; font-size: 13px; line-height: 1.45; }
input::placeholder, textarea::placeholder { color: var(--pale); }
input:focus, select:focus, textarea:focus {
  outline: none; border-color: var(--accent); box-shadow: 0 0 0 3.5px var(--anneau);
}
input[type=checkbox], input[type=radio] { accent-color: var(--accent); width: 14px; height: 14px; box-shadow: none; }
label { font-size: 13px; color: var(--encre-douce); font-weight: 400; }

.champ-court { width: 90px; }
.champ-moyen { width: 170px; }
.champ-large { min-width: 260px; }
.pleine { width: 100%; }

.champ { display: flex; flex-direction: column; gap: 5px; }
.champ > label { font-size: 12px; font-weight: 500; color: var(--muet); }
.champ input, .champ select, .champ textarea { width: 100%; }

/* ===== pastilles ======================================================= */
.pastille {
  display: inline-flex; align-items: center; gap: 5px;
  font-size: 11.5px; font-weight: 500; padding: 2px 8px;
  border-radius: 999px; border: 0.5px solid transparent; white-space: nowrap;
}
.pastille::before { content: ""; width: 5px; height: 5px; border-radius: 50%; background: currentColor; }
.p-brouillon { background: var(--surface-3); color: var(--muet); border-color: var(--trait-fort); }
.p-validee { background: var(--bleu-clair); color: var(--bleu); border-color: var(--bleu-trait); }
.p-payee { background: var(--vert-clair); color: var(--vert); border-color: var(--vert-trait); }
.p-retard { background: var(--rouge-clair); color: var(--rouge); border-color: var(--rouge-trait); }
.p-attente { background: var(--ambre-clair); color: var(--ambre); border-color: var(--ambre-trait); }

/* ===== messages ======================================================== */
.flash, .erreur, .alerte {
  display: flex; align-items: center; gap: 9px;
  padding: 10px 13px; margin: 0 0 var(--e4);
  border-radius: 10px; font-size: 13px; font-weight: 400;
  border: 0.5px solid transparent; box-shadow: var(--ombre-1);
}
.flash { background: var(--vert-clair); border-color: var(--vert-trait); color: var(--vert); }
.erreur { background: var(--rouge-clair); border-color: var(--rouge-trait); color: var(--rouge); }
.alerte.info { background: var(--bleu-clair); border-color: var(--bleu-trait); color: var(--bleu); }
.alerte.attention { background: var(--ambre-clair); border-color: var(--ambre-trait); color: var(--ambre); }

/* ===== aides =========================================================== */
.aide { color: var(--muet); font-size: 12.5px; }
.aide.petite { font-size: 11.8px; }
.aide.sous-champ { margin: 5px 0 0; }
.aide.bloc { margin: 0 0 var(--e3); }
.aide.haut { margin-top: 9px; }
.muet { color: var(--muet); }
.haut { margin-top: 9px; }
.rouge { color: var(--rouge); }
.en-ligne, form.en-ligne { display: inline; }
.petite { font-size: 12.2px; }
h1 .petite { font-weight: 400; color: var(--muet); letter-spacing: 0; }

/* ===== états vides ===================================================== */
.vide {
  display: flex; flex-direction: column; align-items: center; gap: 7px;
  color: var(--muet); padding: var(--e6) var(--e4); text-align: center;
  background: var(--surface); border: 1px dashed var(--trait-fort);
  border-radius: var(--rayon-lg); font-size: 13px;
}
.vide strong { color: var(--encre-douce); font-weight: 600; }
.vide a { font-weight: 500; }
.vide .geste { color: var(--pale); font-size: 12px; }

/* ===== fiches ========================================================== */
.fiche { display: grid; grid-template-columns: 2fr 1fr; gap: var(--e5); align-items: start; }
.fiche dl {
  margin: 0; background: var(--surface); border: 0.5px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 4px 16px; box-shadow: var(--ombre-1);
}
.fiche dt { font-size: 11px; letter-spacing: .01em; color: var(--pale); margin-top: 12px; font-weight: 500; }
.fiche dd { margin: 2px 0 12px; font-size: 13.5px; }
.total-fort { font-size: 21px; font-weight: 600; font-variant-numeric: tabular-nums; letter-spacing: -.03em; }

/* ===== tableau de bord : barres ======================================= */
.curve {
  background: var(--surface); border: 0.5px solid var(--trait);
  border-radius: var(--rayon-lg); padding: 15px 17px 9px; box-shadow: var(--ombre-1);
}
.curve table { border: none; box-shadow: none; border-radius: 0; }
.curve th, .curve td { border: none; padding: 4px 6px; font-size: 12px; background: none; }
.curve thead th { position: static; font-size: 11px; }
.curve .piste { background: var(--surface-3); height: 8px; width: 100%; border-radius: 999px; overflow: hidden; }
.curve .vente { background: var(--accent); height: 8px; border-radius: 999px; }
.curve .achat { background: var(--trait-fort); height: 8px; border-radius: 999px; }

/* ===== barre d'outils ================================================== */
.barre-outils {
  display: flex; gap: var(--e2); align-items: center; flex-wrap: wrap;
  padding: 8px 10px; background: var(--surface);
  border: 0.5px solid var(--trait); border-radius: 10px;
  margin: 0 0 var(--e3); box-shadow: var(--ombre-1);
}
.barre-outils form { display: flex; gap: var(--e2); align-items: center; flex-wrap: wrap; margin: 0; }
.barre-outils .pousse { margin-left: auto; }

/* ===== compte ========================================================= */
.compte-liste { color: var(--muet); font-size: 12.2px; margin: 0; font-variant-numeric: tabular-nums; }
.compte-liste strong { color: var(--encre); font-weight: 600; font-size: 13px; }

/* ===== connexion ======================================================= */
.connexion { min-height: calc(100vh - 240px); display: grid; place-items: center; padding: var(--e5) 0; }
.connexion-carte {
  width: 100%; max-width: 376px; background: var(--surface);
  border: 0.5px solid var(--trait); border-radius: 16px;
  padding: var(--e5) var(--e5) var(--e4); box-shadow: var(--ombre-3);
}
.connexion-marque { display: flex; align-items: center; gap: 9px; font-weight: 600; margin-bottom: var(--e4); font-size: 14.5px; }
.connexion-marque .sceau {
  display: grid; place-items: center; width: 30px; height: 30px; border-radius: 8px;
  color: #fff; font-size: 14px; font-weight: 600;
  background: linear-gradient(160deg, #4da3ff 0%, #0071e3 55%, #0058b8 100%);
  box-shadow: inset 0 0.5px 0 rgba(255, 255, 255, .5), 0 2px 6px rgba(0, 0, 0, .2);
}
.connexion h1 { font-size: 22px; margin-bottom: 2px; }
.connexion .sous-titre { margin-bottom: var(--e4); }
.connexion form { display: flex; flex-direction: column; gap: var(--e3); }
.connexion input { height: 34px; }

/* ===== adaptatif ======================================================= */
@media (max-width: 1080px) {
  .contenu { padding: var(--e4) var(--e4) 0; }
  header input[type=search] { width: 180px; }
  header input[type=search]:focus { width: 220px; }
}
@media (max-width: 900px) {
  header { padding: 0 10px; gap: var(--e2); flex-wrap: wrap; min-height: 0; }
  header nav { order: 3; width: 100%; padding-bottom: 6px; }
  header nav a { font-size: 12.5px; }
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
    border-right: none; border-bottom: 0.5px solid var(--trait); padding: 6px 8px;
    background: var(--surface);
  }
  .menu-gauche .module-titre, .menu-gauche hr { display: none; }
  .menu-gauche a, .menu-gauche span.avenir { white-space: nowrap; padding: 5px 9px; }
  .menu-gauche a svg { display: none; }
  thead th { position: static; }
  h1 { font-size: 23px; }
}
@media (max-width: 640px) {
  .cartes, .kpis { grid-template-columns: 1fr 1fr; }
  .carte .valeur, .kpi .valeur { font-size: 20px; }
}

/* ===== mode sombre ===================================================== */
@media (prefers-color-scheme: dark) {
  :root {
    --encre: #f5f5f7;
    --encre-douce: #d2d2d7;
    --muet: #98989d;
    --pale: #6e6e73;
    --papier: #131316;
    --surface: #1c1c1f;
    --surface-2: #202024;
    --surface-3: #2a2a2e;
    --verre: rgba(28, 28, 31, .72);
    --trait: rgba(255, 255, 255, .08);
    --trait-fort: rgba(255, 255, 255, .16);
    --accent: #0a84ff;
    --accent-fonce: #3d9bff;
    --accent-clair: rgba(10, 132, 255, .16);
    --accent-trait: rgba(10, 132, 255, .34);
    --anneau: rgba(10, 132, 255, .45);
    --vert: #32d583;
    --vert-clair: rgba(50, 213, 131, .14);
    --vert-trait: rgba(50, 213, 131, .3);
    --rouge: #ff6961;
    --rouge-clair: rgba(255, 105, 97, .14);
    --rouge-trait: rgba(255, 105, 97, .3);
    --bleu: #64a8ff;
    --bleu-clair: rgba(100, 168, 255, .14);
    --bleu-trait: rgba(100, 168, 255, .3);
    --ambre: #ffcc66;
    --ambre-clair: rgba(255, 204, 102, .14);
    --ambre-trait: rgba(255, 204, 102, .3);
    --ombre-1: 0 0.5px 1px rgba(0, 0, 0, .5);
    --ombre-2: 0 6px 18px rgba(0, 0, 0, .5);
    --ombre-3: 0 20px 44px rgba(0, 0, 0, .7);
  }
  .menu-gauche { background: rgba(24, 24, 27, .8); }
  button, .bouton { color: #fff; }
  .bouton.secondaire, button.secondaire, .bouton.fantome, button.fantome { color: var(--encre-douce); }
}
"#;
