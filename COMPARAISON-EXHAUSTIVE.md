# Comparaison exhaustive : noyau Rust contre Dolibarr 24.0.1

Mesures du **3 novembre 2026**. Les chiffres Dolibarr viennent de son dépôt
officiel (tag 24.0.1), comptés sur pièce, pas estimés. Les chiffres Rust viennent
du dépôt local.

## 1. Le volume

| | Dolibarr 24.0.1 | Noyau Rust | Rapport |
|---|---|---|---|
| Fichiers | 15 640 | 94 `.rs` | 1 / 166 |
| Lignes de code | **1 630 232** de PHP | **32 790** de Rust | **2,0 %** |
| Fichiers PHP dans `htdocs` | 4 469 | — | — |
| Crates / paquets | — | 19 | — |
| Écrans | 593 pages de module + 196 pages d'admin = **789** | **112** routes HTML | **14 %** |
| Endpoints API | ~200 | 15 | 7,5 % |
| Tests | ~4 000 | **269** | 6,7 % |

Deux lignes à lire ensemble : **2 % du code pour 14 % des écrans**. C'est le seul
chiffre qui compte — Rust dit plus par ligne, mais un écran Dolibarr reste plus
profond que le nôtre.

## 2. Ce qui est identique, et vérifiable

| Élément | État | Preuve |
|---|---|---|
| Schéma SQL | **410 tables, identiques à l'octet** | `schema verify` → `compatible` (empreinte `7293039a…`) |
| Constantes `MAIN_MODULE_*` | mêmes noms | semis en base, 94 modules |
| Droits (`llx_rights_def`) | **mêmes identifiants** : 11 facture, 16 paiement, 21 devis, 41/141 projet, 81 commande, 95 compta, 111/114 banque, 241/242 tiers, 1181 fournisseur, 56001 ticket, 2501 document, mais aussi 221 mailing, 331 signet, 511/512 salaire, 521 prêt, 1152 commande fournisseur, 151 prélèvement, 561 virement, 4001 RH, 51001 immobilisation, 63001 ressource, 50411 grand livre | écrits en base, testés |
| Menus (`llx_menu`) | lus depuis la table, ordre Dolibarr respecté | 12 menus de tête importés (Home, Members, ThirdParties, ProductsPipeServices, MRP, Projects, Commercial, MenuFinancial, MenuBankCash, MenuAccountancy, HRM, Tools) |
| Arithmétique des prix | identique au centime | `calcul_price_total()` PHP conservée comme oracle de test |
| Mots de passe | bcrypt `$2y$` | un compte créé ici fonctionne dans Dolibarr et l'inverse |
| Multi-entité | par session | entité vérifiée sur 410 tables |

## 3. Les écrans, module par module

### Couverts avec écriture complète (36 modules à écran)

Tiers, Contacts, Produits, Services, Stock, Devis, Commandes, Commandes
fournisseurs, Contrats, Expéditions, Factures, Avoirs, Règlements, Achats,
Chantiers (projets et tâches), Tickets, Banques, Documents, Agenda,
Comptabilité, Paramètres, Utilisateurs, Import/Export, Adhérents, Catégories,
Dons, Prêts, Immobilisations, Lots, Ressources, Taux de TVA, Réceptions,
Notes de frais, Congés, Emailings, Prélèvements, Virements, Fiches
d'intervention, Signets, Modèles d'e-mails, Profils expéditeurs, Pays,
Propositions fournisseurs.

Sur les **37 modules Dolibarr qui déclarent un écran**, 36 ont le leur. Le 37ᵉ
est Mailman/SPIP : aucune table, rien à lire.

### Écrans qui existent en Rust et pas dans Dolibarr de base

- explorateur de **410 tables** (lecture générique),
- diagnostic d'installation,
- **téléchargement du bordereau SEPA** depuis la liste des factures à payer,
- **campagnes d'e-mailing** avec cible et essai,
- liste des **greffons** chargés.

### Absents, et qui comptent

| Domaine | Ce qui manque |
|---|---|
| **Point de vente** | écran tactile, session de caisse persistée, journal des ventes, ticket PDF |
| **MRP** | besoins nets, stocks, délais, ordres proposés — seule l'explosion BOM est faite |
| **Paie** | barème officiel, cotisations patronales, cumuls, proratisation, PAS |
| **Comptabilité tenue** | écritures, lettrage, à-nouveaux, clôture. Le grand livre est **dérivé** des documents |
| **Récurrences** | devis, factures, contrats, abonnements |
| **Extrafields** | les données existent, aucun écran ne les montre |
| **Journal des événements** | qui a fait quoi, quand, sur quel objet |
| **Actions de masse** | sélectionner 40 factures et les valider |
| **Modèles d'e-mails par objet** | le lien modèle ↔ moment d'envoi |
| **Multidevises dans les documents** | taux figé sur la pièce |
| **Constructeur de site, portails, TakePOS, notifications, LDAP/OAuth, cron, workflow** | rien |

## 4. Les moteurs

| Moteur | Dolibarr | Rust |
|---|---|---|
| Calcul des prix | `calcul_price_total()` | fait, décimal exact, vérifié |
| TVA sur encaissements | oui | fait |
| Factures de situation | oui (module) | fait (cycle, sommes au centime) |
| SEPA | module `modVirement` / `modPrelevement` | fait, `pain.001` + `pain.008`, IBAN modulo 97 |
| E-mailing | `modMailing` + PHPMailer | fait, SMTP direct + TLS, multipart, pièces jointes |
| PDF | 6 modèles par objet × 4 objets | 3 modèles, 2 gabarits distincts |
| MRP | `modMrp` complet | explosion BOM seule |
| Caisse | `modTakePos` | panier, TVA ligne, remise, rendu, clôture, ticket texte |
| Paie | via modules externes | calcul vide, barème fourni en argument |
| Comptabilité | écritures réelles | dérivé, pas tenu |

## 5. Architecture et extensibilité

| | Dolibarr | Rust |
|---|---|---|
| Modules | PHP, `$this->menu = 1`, hooks | déclaratifs (JSON, sans recompiler) + greffons ABI C |
| Ajouter un module | écrire du PHP, activer | déposer un `.json`, ou compiler un `.dll` |
| Modules Dolistore | tournent | **ne tournent pas** — aucun PHP exécuté |
| Thèmes | eldy, auguria | un seul, CSS en dur |
| Traductions | 30+ langues, fichiers `.lang` | **français uniquement, en dur** |

## 6. Exploitation

| | Dolibarr | Rust |
|---|---|---|
| Installation | assistant web (1 097 fichiers) | `installer.ps1` + `/installation` (diagnostic + création de l'admin) |
| Dépendances serveur | PHP 8+, extensions, MySQL | **binaire unique**, MariaDB |
| Mémoire par requête | processus PHP | ~10 Mo, un seul processus |
| Latence mesurée | non mesurée ici | **2 à 9 ms** sur la base de test |
| Mode lecture seule | non | `DOLIBARR_LECTURE_SEULE=1` → 403 sur toute écriture |
| Journal | `llx_events`, journaux PHP | `erreurs.log` + écran |
| Sauvegarde | module + cron | `dump.ps1` (structure claire + données chiffrées) |

## 7. Ce que la comparaison ne dit pas

1. **La profondeur d'un écran.** Une fiche facture Dolibarr porte les actions de
   masse, les récurrences, l'envoi par modèle, l'allocation des paiements, le
   journal des événements, les extrafields. La nôtre : créer, valider, encaisser,
   PDF, situation. Un écran présent n'est pas un écran complet.
2. **La couverture linguistique.** Français seulement. Dolibarr en sert 30+.
3. **L'écosystème.** 0 module tiers compatible contre plusieurs centaines.
4. **La durée.** Dolibarr a 20 ans de corrections. Ce noyau a 3 semaines.

## 8. Verdict

**Le même logiciel, non. La même base, oui.**

Ce noyau parle la langue de Dolibarr — schéma, droits, constantes, menus,
arithmétique, mots de passe — et il le fait en 2 % du code. Il couvre 14 % des
écrans, avec une profondeur inégale, et **aucun** module tiers.

Ce qui est réellement acquis :

- une base interchangeable dans les deux sens ;
- la preuve qu'un noyau d'ERP peut s'écrire en Rust sans trahir les données ;
- une architecture d'extension (JSON + ABI C) que Dolibarr n'a pas ;
- 269 tests qui échouent quand quelque chose casse.

Ce qui reste à faire pour prétendre au titre : la comptabilité tenue, la paie
réelle, le point de vente, le MRP complet, les extrafields, les récurrences, et
les traductions. C'est-à-dire **98 % du code**, mais plus la partie structurante.
