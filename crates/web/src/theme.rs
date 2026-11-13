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
/// Les styles proposes : cle technique, nom affiche, intention.
///
/// Une seule liste : le selecteur de l'en-tete, la page de choix et les apercus
/// la lisent. Ajouter un style, c'est ajouter une ligne ici et un bloc de
/// jetons plus bas — rien d'autre.
pub const STYLES: &[(&str, &str, &str)] = &[
    (
        "systeme",
        "Style système",
        "Suit le réglage clair/sombre de la machine. Le choix par défaut.",
    ),
    (
        "collaboratif",
        "Collaboratif",
        "Angles doux, accent bleu, beaucoup d'air. Pour travailler à plusieurs.",
    ),
    (
        "comptable",
        "Comptable",
        "Chiffres alignés, traits nets, vert sobre. Pour lire des totaux.",
    ),
    (
        "futuriste",
        "Futuriste",
        "Fond profond, halos froids, accent cyan. Pour les longues sessions.",
    ),
    (
        "minuit",
        "Minuit",
        "Sombre et dense, sans reflet. Pour l'atelier ou la nuit.",
    ),
    (
        "sable",
        "Sable",
        "Papier chaud, accent terre cuite. Le plus proche du devis imprimé.",
    ),
    (
        "classique",
        "Classique",
        "Gris neutre, bordures franches. L'apparence d'un logiciel de gestion.",
    ),
];

/// La feuille des apercus : les memes blocs, portes par `.apercu` au lieu de
/// `html`.
///
/// Un aperçu a besoin des jetons complets ; les dupliquer a la main serait la
/// garantie qu'ils divergent. On les reecrit donc mecaniquement.
pub fn css_apercus() -> String {
    CSS.replace("html[data-style=\"", ".apercu[data-style=\"")
}

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

/* Quatre utilitaires qui manquaient : ils reviennent partout en style inline. */
.code { font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 13px; }
.rangee { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.ecart { margin-left: 8px; }
.defile { overflow-x: auto; }
.zone-texte { min-height: 96px; font-size: 15px; }

/* Dernier passage : ce qui restait ecrit en dur sort du theme.
   Une couleur en dur ne suivrait pas « Minuit » ; une largeur, si. */
.taille-14 { font-size: 14px; }
.minuscule { font-size: 11px; }
.bloc { display: block; }
.pre { white-space: pre-wrap; }
.etiquette { display: inline-block; margin: 0 8px 8px 0; }
.ecart-droit { margin-right: 6px; }
.marge-basse { margin: 0 0 14px; }
.espace-bas { margin-bottom: 22px; }
.espace-dessus { margin-top: 18px; }
.sans-marge { margin-top: 0; }
.etroit { width: 100px; }
.panneau { background: var(--surface-2); border: 1px solid var(--trait); border-radius: var(--rayon); padding: 14px; }
.panneau.note { font-size: 13.5px; white-space: pre-wrap; margin: 0; }
.bloc-alerte { background: var(--rouge-clair); border-left: 3px solid var(--rouge); padding: 10px 12px; margin: 0 0 16px; }
.bouton-blanc { background: var(--surface); color: var(--encre); border: 1px solid var(--trait); padding: 12px 18px; font-size: 15px; }
.bouton-danger { background: var(--rouge); color: #fff; border: 0; padding: 9px 16px; cursor: pointer; }
.temoin { display: inline-block; width: 10px; height: 10px; margin-right: 5px; background: var(--accent); }
.temoin.achat { background: var(--trait-fort); }
.piste-basse { background: var(--surface-3); height: 10px; margin-top: 4px; }
.geste-lien { color: var(--muet); font-size: 13px; margin-left: 10px; }
.ligne-total { background: var(--surface-3); font-weight: 600; }
.texte-vert { color: var(--vert); }
.zone-code { width: 100%; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; padding: 10px; border: 1px solid var(--trait); }
.grand-champ { padding: 14px 16px; font-size: 16px; }
.grand-champ.grand { padding: 18px 22px; width: 100%; }
.grille { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 14px; }
.grille-vente { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 14px; align-items: end; max-width: 900px; }
.ligne-ecart { display: flex; justify-content: space-between; gap: 10px; align-items: baseline; }
.rangee.large { margin-top: 16px; gap: 14px; align-items: end; }
.aide.dessous { margin: 8px 0 0; }
.aide.monte { margin: 12px 0 0; }
.aide.retrait { font-size: 13px; margin: -14px 0 14px; }
.aide.apres { margin: 10px 0 0; }

/* Largeurs des colonnes de graphique. Statiques, donc classes. */
.col-46 { width: 46%; }
.col-32 { width: 32%; }
.col-30 { width: 30%; }
.col-28 { width: 28%; }
.col-26 { width: 26%; }
.col-20 { width: 20%; }

.reglage { border: 1px solid var(--trait); border-radius: var(--rayon); padding: 8px 12px; margin: var(--e3) 0; background: var(--surface-2); }
.reglage > summary { cursor: pointer; font-size: 12.5px; color: var(--muet); }
.reglage[open] > summary { margin-bottom: 8px; }

/* ===== tableau de bord ================================================= */
/* Le bandeau : le seul endroit de l'application ou l'on prend de la place. */
.hero {
  display: flex; gap: var(--e4); align-items: flex-end; justify-content: space-between;
  flex-wrap: wrap;
  padding: var(--e5) var(--e5) var(--e4);
  border-radius: 18px;
  background:
    radial-gradient(1200px 200px at 0% 0%, var(--accent-clair), transparent 70%),
    var(--surface);
  border: 1px solid var(--trait);
  box-shadow: var(--ombre-1);
  margin-bottom: var(--e4);
}
.hero h1 { margin: 0; font-size: 26px; letter-spacing: -.02em; }
.surtitre {
  margin: 0 0 4px; font-size: 11px; font-weight: 600;
  letter-spacing: .12em; text-transform: uppercase; color: var(--muet);
}
.hero-date { margin: 4px 0 0; font-size: 13px; color: var(--muet); }
.hero-actions { display: flex; gap: var(--e2); flex-wrap: wrap; }

/* Les widgets : une surface, un entete, un corps. */
.grille-2 {
  display: grid; grid-template-columns: 1fr 1fr; gap: var(--e4);
  margin-bottom: var(--e4);
}
.widget {
  background: var(--surface); border: 1px solid var(--trait);
  border-radius: 16px; padding: var(--e4);
  box-shadow: var(--ombre-1);
  min-width: 0;               /* sinon un tableau large fait exploser la grille */
}
.widget-entete {
  display: flex; align-items: center; justify-content: space-between;
  gap: var(--e2); margin-bottom: var(--e3);
}
.widget-entete h2 { margin: 0; font-size: 14px; font-weight: 600; letter-spacing: -.01em; }
.widget-sous { font-size: 11.5px; color: var(--muet); }
.widget-lien { font-size: 12px; color: var(--accent); text-decoration: none; }
.widget-lien:hover { text-decoration: underline; }
.widget-vide { margin: 0; font-size: 13px; color: var(--muet); }

/* Les listes de choses a traiter : une ligne, un lien, un montant a droite. */
.liste-alertes { list-style: none; margin: 0; padding: 0; }
.liste-alertes li {
  display: flex; align-items: baseline; gap: var(--e2);
  padding: 7px 0; border-bottom: 1px solid var(--trait);
  font-size: 13px;
}
.liste-alertes li:last-child { border-bottom: 0; }
.liste-alertes a { font-weight: 500; text-decoration: none; color: var(--encre); }
.liste-alertes a:hover { color: var(--accent); }
.liste-detail { color: var(--muet); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.liste-droit { margin-left: auto; font-variant-numeric: tabular-nums; color: var(--encre-douce); }

/* Un tableau dense, sans entete : il tient dans un widget. */
.tableau-serre { width: 100%; font-size: 13px; }
.tableau-serre td { padding: 7px 6px; border-bottom: 1px solid var(--trait); }
.tableau-serre tr:last-child td { border-bottom: 0; }
.tableau-serre td.num { text-align: right; font-variant-numeric: tabular-nums; }

/* Le bandeau du bas : quatre mesures, petites, alignees. */
.bandeau-bas {
  display: flex; flex-wrap: wrap; gap: var(--e4);
  padding: var(--e3) var(--e4);
  border-radius: 12px; border: 1px solid var(--trait);
  background: var(--surface-2);
  font-size: 12px; color: var(--muet);
}
.bandeau-bas strong { color: var(--encre); font-weight: 600; font-variant-numeric: tabular-nums; }

@media (max-width: 1000px) {
  .grille-2 { grid-template-columns: 1fr; }
}
@media (max-width: 640px) {
  .hero { padding: var(--e3); border-radius: 14px; }
  .hero h1 { font-size: 21px; }
  .hero-actions { width: 100%; }
  .hero-actions .bouton { flex: 1 1 auto; text-align: center; }
}

/* ===== tableaux ======================================================== */
table {
  width: 100%; border-collapse: separate; border-spacing: 0;
  background: var(--surface); border: 0.5px solid var(--trait);
  border-radius: var(--rayon-lg); overflow: hidden;
  box-shadow: var(--ombre-1); font-size: 13.5px;
}
thead th {
  /* Pas de `position: sticky` ici : dans un tableau a coins arrondis
     (`overflow: hidden`), Chromium decalait l'en-tete de 52 px et il recouvrait
     la premiere ligne — une liste de 5 devis n'en montrait que 4. */
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

@media (max-width: 720px) {
  /* Un tableau plus large que l'ecran defile dans son propre cadre : la page,
     elle, ne bouge pas horizontalement. */
  main table {
    display: block; width: 100%; overflow-x: auto;
    -webkit-overflow-scrolling: touch;
  }
  main table thead, main table tbody { display: table; width: 100%; min-width: 520px; }
  .widget { padding: var(--e3); border-radius: 13px; }
  .bandeau-bas { gap: var(--e3); }
}

/* ===== page de style =================================================== */
.grille-styles {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: var(--e3); margin: var(--e3) 0 var(--e4);
}
.style-carte {
  display: block; cursor: pointer; position: relative;
  border: 1px solid var(--trait); border-radius: 14px; overflow: hidden;
  background: var(--surface); transition: box-shadow var(--duree) var(--courbe), border-color var(--duree);
}
.style-carte:hover { box-shadow: var(--ombre-2); }
.style-carte:has(input:checked) { border-color: var(--accent-trait); box-shadow: 0 0 0 2px var(--accent-clair); }
.style-carte input { position: absolute; top: 10px; right: 10px; z-index: 2; }

/* L'aperçu : un vrai bout de l'application, dans les jetons du style. */
.apercu {
  background: var(--papier); color: var(--encre);
  border-bottom: 1px solid var(--trait);
  padding: 12px; font-size: 12px;
  display: flex; flex-direction: column; gap: 8px;
  height: 158px; overflow: hidden;
}
.apercu-titre { font-size: 10.5px; letter-spacing: .1em; text-transform: uppercase; color: var(--muet); }
.apercu-kpi {
  background: var(--surface); border: 1px solid var(--trait); border-radius: 10px;
  padding: 8px 10px; display: flex; flex-direction: column; gap: 2px;
}
.apercu-titre-kpi { font-size: 10px; color: var(--muet); }
.apercu-valeur { font-size: 17px; font-weight: 600; letter-spacing: -.03em; color: var(--accent); font-variant-numeric: tabular-nums; }
.apercu-ligne { display: flex; align-items: center; justify-content: space-between; }
.apercu-nombre { font-variant-numeric: tabular-nums; color: var(--encre-douce); }
.apercu-actions { display: flex; gap: 6px; }
.apercu-actions .bouton { padding: 4px 9px; font-size: 11px; border-radius: 6px; }

.style-texte { padding: 10px 12px 12px; display: flex; flex-direction: column; gap: 2px; }
.style-texte strong { font-size: 13px; }
.style-texte span { font-size: 11.5px; color: var(--muet); line-height: 1.35; }

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
  /* Le tiroir : hors ecran par defaut, il glisse quand la case est cochee. */
  .menu-gauche {
    position: fixed; inset: 0 auto 0 0; z-index: 40;
    width: 264px; max-width: 84vw; height: 100%;
    display: block; overflow-y: auto;
    transform: translateX(-102%);
    transition: transform var(--duree) var(--courbe);
    box-shadow: var(--ombre-3);
    padding: var(--e3) 10px;
  }
  .bascule:checked ~ .corps .menu-gauche { transform: translateX(0); }
  .bascule:checked ~ .corps .voile {
    position: fixed; inset: 0; z-index: 35;
    background: rgba(0, 0, 0, .38); backdrop-filter: blur(1.5px);
  }
  .bascule-bouton { display: inline-flex; }
  .menu-gauche a, .menu-gauche span.avenir { padding: 9px 10px; font-size: 13.5px; }
  .menu-gauche a svg, .menu-gauche span.avenir svg { display: inline-block; }
  thead th { position: static; }
  h1 { font-size: 23px; }
}
@media (max-width: 640px) {
  .cartes, .kpis { grid-template-columns: 1fr 1fr; }
  .carte .valeur, .kpi .valeur { font-size: 20px; }
}

/* ===== styles choisis par l'utilisateur ================================
   Trois directions, plus celle du système. Elles ne changent que des jetons :
   aucune page n'est reprise, aucun écran n'est dupliqué. `data-style` est posé
   sur `<html>` ; sans attribut, c'est le style « systeme » qui s'applique.
   ====================================================================== */

/* « futuriste » : fond profond, halos froids, accent cyan. */
html[data-style="futuriste"] {
  --encre: #e8eaf6;
  --encre-douce: #c3c8e6;
  --muet: #8f96c4;
  --pale: #6b729e;
  --papier: #0a0b14;
  --surface: #12131f;
  --surface-2: #171929;
  --surface-3: #1f2234;
  --verre: rgba(18, 19, 31, .78);
  --trait: rgba(120, 140, 255, .12);
  --trait-fort: rgba(120, 140, 255, .26);
  --accent: #22d3ee;
  --accent-fonce: #67e8f9;
  --accent-clair: rgba(34, 211, 238, .14);
  --accent-trait: rgba(34, 211, 238, .34);
  --anneau: rgba(34, 211, 238, .4);
  --vert: #4ade80;
  --vert-clair: rgba(74, 222, 128, .13);
  --vert-trait: rgba(74, 222, 128, .3);
  --rouge: #fb7185;
  --rouge-clair: rgba(251, 113, 133, .13);
  --rouge-trait: rgba(251, 113, 133, .3);
  --bleu: #7dd3fc;
  --bleu-clair: rgba(125, 211, 252, .13);
  --bleu-trait: rgba(125, 211, 252, .3);
  --ambre: #fbbf24;
  --ambre-clair: rgba(251, 191, 36, .13);
  --ambre-trait: rgba(251, 191, 36, .3);
  --ombre-1: 0 0 0 0.5px rgba(120, 140, 255, .1), 0 0 24px rgba(34, 211, 238, .06);
  --ombre-2: 0 0 0 0.5px rgba(120, 140, 255, .16), 0 8px 32px rgba(34, 211, 238, .1);
  --ombre-3: 0 0 0 1px rgba(120, 140, 255, .2), 0 24px 60px rgba(0, 0, 0, .6);
}
html[data-style="futuriste"] header .marque .sceau,
html[data-style="futuriste"] .connexion-marque .sceau {
  background: linear-gradient(160deg, #22d3ee 0%, #6366f1 100%);
  box-shadow: 0 0 18px rgba(34, 211, 238, .5);
}
html[data-style="futuriste"] h1 { letter-spacing: -.045em; }

/* « classique » : papier, encre, aucune couleur d'action vive. Le Dolibarr
   d'avant, resserre : celui qui ne deplace personne. */
html[data-style="classique"] {
  --encre: #1f1f1f;
  --encre-douce: #3d3d3d;
  --muet: #666;
  --pale: #8c8c8c;
  --papier: #f4f4f4;
  --surface: #ffffff;
  --surface-2: #fafafa;
  --surface-3: #f0f0f0;
  --verre: #ffffff;
  --trait: #dddddd;
  --trait-fort: #c4c4c4;
  --accent: #264d73;
  --accent-fonce: #1c3a57;
  --accent-clair: #e8eef4;
  --accent-trait: #c3d3e0;
  --anneau: rgba(38, 77, 115, .3);
  --vert: #2f6b45;
  --vert-clair: #eaf3ee;
  --vert-trait: #cbe0d3;
  --rouge: #a33a2a;
  --rouge-clair: #fbecea;
  --rouge-trait: #efd3cd;
  --bleu: #264d73;
  --bleu-clair: #e8eef4;
  --bleu-trait: #c3d3e0;
  --ambre: #8a6a1f;
  --ambre-clair: #fbf3de;
  --ambre-trait: #ecdfb9;
  --rayon: 2px;
  --rayon-lg: 2px;
  --ombre-1: none;
  --ombre-2: none;
  --ombre-3: none;
}
html[data-style="classique"] header { backdrop-filter: none; border-bottom: 1px solid #c4c4c4; }
html[data-style="classique"] .menu-gauche { backdrop-filter: none; background: #fff; }
html[data-style="classique"] header nav a.actif { background: transparent; border-bottom: 2px solid var(--accent); border-radius: 0; }
html[data-style="classique"] table { border: 1px solid #c4c4c4; }
html[data-style="classique"] thead th { background: #f0f0f0; border-bottom: 1px solid #c4c4c4; }
html[data-style="classique"] td { border-bottom: 1px solid #e6e6e6; }
html[data-style="classique"] h1 { font-size: 22px; letter-spacing: 0; font-weight: 600; }

/* « minuit » : sombre neutre et dense, pour travailler tard sans agresser
   l'oeil. Pas de halo, pas de couleur criarde : du gris, un accent sourd. */
html[data-style="minuit"] {
  --encre: #e9e9ec;
  --encre-douce: #c2c2c7;
  --muet: #8e8e94;
  --pale: #6a6a70;
  --papier: #0e0e10;
  --surface: #17171a;
  --surface-2: #1c1c20;
  --surface-3: #26262b;
  --verre: rgba(23, 23, 26, .8);
  --trait: rgba(255, 255, 255, .07);
  --trait-fort: rgba(255, 255, 255, .14);
  --accent: #c9a227;
  --accent-fonce: #e0b63a;
  --accent-clair: rgba(201, 162, 39, .14);
  --accent-trait: rgba(201, 162, 39, .32);
  --anneau: rgba(201, 162, 39, .4);
  --vert: #56c98f;
  --vert-clair: rgba(86, 201, 143, .13);
  --vert-trait: rgba(86, 201, 143, .28);
  --rouge: #f07167;
  --rouge-clair: rgba(240, 113, 103, .13);
  --rouge-trait: rgba(240, 113, 103, .28);
  --bleu: #7aa7d9;
  --bleu-clair: rgba(122, 167, 217, .13);
  --bleu-trait: rgba(122, 167, 217, .28);
  --ambre: #e3b341;
  --ambre-clair: rgba(227, 179, 65, .13);
  --ambre-trait: rgba(227, 179, 65, .28);
  --ombre-1: 0 0.5px 1px rgba(0, 0, 0, .6);
  --ombre-2: 0 6px 20px rgba(0, 0, 0, .5);
  --ombre-3: 0 22px 50px rgba(0, 0, 0, .7);
}
html[data-style="minuit"] header .marque .sceau,
html[data-style="minuit"] .connexion-marque .sceau {
  background: linear-gradient(160deg, #e0b63a 0%, #9a7b1c 100%);
  color: #14140f;
}

/* « sable » : clair et chaud, papier de calque, encre brune, accent terre
   cuite. Le contemporain des maisons d'édition et des cabinets. */
html[data-style="sable"] {
  --encre: #211d19;
  --encre-douce: #423a32;
  --muet: #6b6157;
  --pale: #94897d;
  --papier: #f6f2ec;
  --surface: #fffdfa;
  --surface-2: #fbf7f0;
  --surface-3: #f1ebe1;
  --verre: rgba(255, 253, 250, .8);
  --trait: rgba(84, 66, 46, .12);
  --trait-fort: rgba(84, 66, 46, .22);
  --accent: #b4562a;
  --accent-fonce: #96451f;
  --accent-clair: rgba(180, 86, 42, .1);
  --accent-trait: rgba(180, 86, 42, .28);
  --anneau: rgba(180, 86, 42, .32);
  --vert: #3d7a53;
  --vert-clair: rgba(61, 122, 83, .1);
  --vert-trait: rgba(61, 122, 83, .24);
  --rouge: #a93b2c;
  --rouge-clair: rgba(169, 59, 44, .09);
  --rouge-trait: rgba(169, 59, 44, .24);
  --bleu: #2f5d78;
  --bleu-clair: rgba(47, 93, 120, .09);
  --bleu-trait: rgba(47, 93, 120, .22);
  --ambre: #8c6a1f;
  --ambre-clair: rgba(140, 106, 31, .1);
  --ambre-trait: rgba(140, 106, 31, .24);
  --rayon: 6px;
  --rayon-lg: 10px;
}
html[data-style="sable"] header .marque .sceau,
html[data-style="sable"] .connexion-marque .sceau {
  background: linear-gradient(160deg, #cf7844 0%, #96451f 100%);
}
html[data-style="sable"] h1 { letter-spacing: -.025em; }

/* Tableaux : sous 720 px, le tableau defile horizontalement dans son cadre
   plutot que de deborder de la page. Une colonne ne se coupe pas. */
@media (max-width: 720px) {
  table { display: block; overflow-x: auto; -webkit-overflow-scrolling: touch; }
  thead, tbody, tfoot { display: table; width: 100%; min-width: 560px; }
  .kpis, .cartes { grid-template-columns: 1fr; }
  .actions { gap: 6px; }
  .actions .bouton, .actions button { flex: 1 1 auto; }
  input[type=text], input[type=number], input[type=email], input[type=date],
  input[type=password], select, textarea { width: 100%; }
  .champ-court, .champ-moyen, .champ-large { width: 100%; min-width: 0; }
  .fiche dl { padding: 4px 12px; }
  h1 { font-size: 20px; }
  .contenu { padding: var(--e3) 10px 0; }
  header form.societe .mention { display: none; }
  header .compte .bouton { padding: 0 8px; }
}


/* « collaboratif » : l'indigo et la lavande des outils d'equipe, rayons plus
   ronds, pastilles vives. Ce qu'on prend a Monday/Asana sans les degrades. */
html[data-style="collaboratif"] {
  --encre: #1e1b4b;
  --encre-douce: #383273;
  --muet: #6366a8;
  --pale: #8e90c4;
  --papier: #f5f3ff;
  --surface: #ffffff;
  --surface-2: #faf9ff;
  --surface-3: #eeecfd;
  --verre: rgba(255, 255, 255, .82);
  --trait: #e6e3fb;
  --trait-fort: #cfcaf5;
  --accent: #6366f1;
  --accent-fonce: #4f46e5;
  --accent-clair: #eef0ff;
  --accent-trait: #d8d9fd;
  --anneau: rgba(99, 102, 241, .35);
  --vert: #059669;
  --vert-clair: #e7f8f1;
  --vert-trait: #b3e6d2;
  --rouge: #e11d48;
  --rouge-clair: #fff1f4;
  --rouge-trait: #fbd0da;
  --bleu: #2563eb;
  --bleu-clair: #eff4ff;
  --bleu-trait: #cfdcfb;
  --ambre: #b45309;
  --ambre-clair: #fff8eb;
  --ambre-trait: #fde3b3;
  --rayon: 10px;
  --rayon-lg: 14px;
  --ombre-1: 0 1px 2px rgba(49, 46, 129, .06), 0 0 0 1px rgba(99, 102, 241, .05);
  --ombre-2: 0 8px 24px rgba(49, 46, 129, .1);
  --ombre-3: 0 24px 48px rgba(49, 46, 129, .18);
}
html[data-style="collaboratif"] header .marque .sceau,
html[data-style="collaboratif"] .connexion-marque .sceau {
  background: linear-gradient(160deg, #818cf8 0%, #6366f1 60%, #4f46e5 100%);
}
html[data-style="collaboratif"] .pastille { font-weight: 600; }

/* « comptable » : la famille Pennylane / Indy / Qonto / Tiime.
   Presque blanc, violet sourd, tres grands rayons, beaucoup d'air, chiffres
   qui portent la page. Pas de bordure : des surfaces posees. */
html[data-style="comptable"] {
  --encre: #111114;
  --encre-douce: #27272a;
  --muet: #71717a;
  --pale: #a1a1aa;
  --papier: #f7f7f9;
  --surface: #ffffff;
  --surface-2: #fbfbfc;
  --surface-3: #f2f2f5;
  --verre: rgba(255, 255, 255, .86);
  --trait: #ececef;
  --trait-fort: #dcdce2;
  --accent: #5b5bd6;
  --accent-fonce: #4747b8;
  --accent-clair: #f0f0fd;
  --accent-trait: #dcdcf8;
  --anneau: rgba(91, 91, 214, .3);
  --vert: #15803d;
  --vert-clair: #eefaf2;
  --vert-trait: #c4ecd3;
  --rouge: #be123c;
  --rouge-clair: #fff1f3;
  --rouge-trait: #f9cdd7;
  --bleu: #1d4ed8;
  --bleu-clair: #eff4ff;
  --bleu-trait: #d3e0fd;
  --ambre: #a16207;
  --ambre-clair: #fef9ec;
  --ambre-trait: #f6e3b6;
  --rayon: 10px;
  --rayon-lg: 16px;
  --ombre-1: 0 1px 2px rgba(17, 17, 20, .04);
  --ombre-2: 0 6px 20px rgba(17, 17, 20, .06);
  --ombre-3: 0 20px 44px rgba(17, 17, 20, .1);
  --e4: 18px; --e5: 28px; --e6: 36px;
}
html[data-style="comptable"] h1 { font-size: 30px; letter-spacing: -.04em; }
html[data-style="comptable"] header { border-bottom: none; box-shadow: 0 1px 0 var(--trait); }
html[data-style="comptable"] .carte,
html[data-style="comptable"] .kpi { padding: 20px 22px; }
html[data-style="comptable"] .kpi .valeur,
html[data-style="comptable"] .carte .valeur { font-size: 28px; letter-spacing: -.04em; }
html[data-style="comptable"] table { border: none; box-shadow: var(--ombre-2); }
html[data-style="comptable"] td { padding: 13px 16px; }
html[data-style="comptable"] button,
html[data-style="comptable"] .bouton { height: 36px; padding: 0 16px; border-radius: 10px; }
html[data-style="comptable"] input, html[data-style="comptable"] select { height: 36px; }
html[data-style="comptable"] .menu-gauche { background: transparent; border-right: none; }
html[data-style="comptable"] header .marque .sceau,
html[data-style="comptable"] .connexion-marque .sceau {
  background: linear-gradient(160deg, #7c7cf0 0%, #5b5bd6 100%);
  border-radius: 10px;
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
