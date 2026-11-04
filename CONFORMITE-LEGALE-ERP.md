# Ce qu'un ERP/CRM doit respecter — état du noyau

Recherche du **4 novembre 2026**. Chaque obligation est citée avec sa source, puis
confrontée à ce que le noyau Rust fait réellement. Quand un texte n'a pas été
vérifié ici, c'est écrit.

---

## 1. Facturation électronique — obligatoire depuis le 1er septembre 2026

**Ce que dit la loi** (source : `entreprendre.service-public.gouv.fr/actualites/A15683`
et `impots.gouv.fr`, consultés le 4/11/2026) :

| Échéance | Obligation |
|---|---|
| **1er septembre 2026** | **Réception** électronique obligatoire pour **toutes** les entreprises assujetties à la TVA établies en France |
| **1er septembre 2026** | **Émission** obligatoire pour les **grandes entreprises et ETI** |
| **1er septembre 2027** | Émission obligatoire pour **toutes** les entreprises |

Les factures passent par une **plateforme agréée** par l'État (ex-PDP),
directement ou via une solution compatible. Le même calendrier porte
l'**e-reporting** (transmission des données de transaction à l'administration).

**État du noyau : absent.**

- aucun format Factur-X / UBL / CII ;
- aucun connecteur de plateforme agréée ;
- aucun e-reporting.

**Effort** : c'est le plus gros écart de la liste. Le format Factur-X est un PDF
avec un XML embarqué — notre générateur PDF sait déjà produire le PDF, il
faudrait y injecter le XML et le valider contre le schéma EN 16931. Compter
plusieurs semaines, et un partenariat pour la plateforme : on ne devient pas PA
en écrivant du code.

---

## 2. Logiciel de caisse — article 286-I-3° bis du CGI

**Ce que dit la loi** (source : `economie.gouv.fr`, page mise à jour le
24/02/2026, et BOFiP `ACTU-2025-00160`) :

Un logiciel de caisse doit garantir quatre propriétés :

1. **inaltérabilité** des données de paiement ;
2. **sécurisation** (accès, traçabilité) ;
3. **conservation** ;
4. **archivage** des données.

La preuve de conformité est soit un **certificat d'un organisme accrédité**
(le seul admis depuis le BOFiP du 01/10/2025), soit — **rétabli par la loi de
finances pour 2026, adoptée le 19 février 2026** — une **attestation individuelle
délivrée par l'éditeur** du logiciel.

Sanction usuelle : **7 500 €** par exercice (à confirmer sur le texte en vigueur).

**État du noyau : non conforme, et il faut le dire.**

- une vente de caisse écrit une facture et un paiement — mais **rien n'empêche
  la modification ou la suppression** d'une vente passée côté base ;
- **aucun journal inaltérable** (pas de chaîne de hachage, pas de cumul
  progressif) ;
- **aucune trace de clôture de caisse** : le comptage du caissier n'est stocké
  nulle part ;
- `statut = 1` sur le paiement n'est pas un verrou.

**Si tu factures au comptoir avec ce noyau, tu es hors cadre.**
Deux issues : ne pas l'utiliser comme caisse (rester sur Dolibarr ou un logiciel
certifié), ou implémenter les quatre propriétés — chaîne de hachage par vente,
journal append-only, verrou de suppression, export d'archivage. C'est faisable,
et c'est un chantier à part entière.

---

## 3. RGPD

**Ce que dit la loi** : le règlement (UE) 2016/679, applicable depuis 2018.
Pour un ERP qui héberge des données clients : base légale par finalité, registre
des traitements, minimisation, durée de conservation, droits des personnes
(accès, rectification, effacement, portabilité), sécurité, notification des
violations sous 72 h, sous-traitance encadrée (art. 28).

**État du noyau : partiel.**

| Point | État |
|---|---|
| Sécurité des accès | **oui** — bcrypt `$2y$`, session, droits par module |
| Cloisonnement par entité | **oui** — vérifié sur les requêtes |
| Droit d'accès et de rectification | **partiel** — les fiches se consultent et s'éditent |
| Effacement | **non** — pas d'anonymisation, pas de droit à l'oubli ; supprimer une fiche casse les liens comptables |
| Export des données | **partiel** — export CSV par écran, pas d'export complet d'une personne |
| Registre des traitements | **non** — à produire, c'est un document, pas du code |
| Journal des accès | **partiel** — `erreurs.log`, pas de journal de lecture |
| Durées de conservation | **non** — rien n'est purgé automatiquement |

**À retenir** : un ERP ne « devient » pas conforme par du code. Le registre, les
mentions et les durées se décident ; le code les rend possibles.

---

## 4. Archivage et FEC

**Ce que dit la loi** :

- **factures** : 10 ans (art. L123-22 du code de commerce) ;
- **comptabilité** : 10 ans (art. L123-22), et le **FEC** (fichier des écritures
  comptables, art. A47 A-1 du LPF) doit pouvoir être produit **pour toute
  période auditée**, y compris en cas de contrôle informatique.

**État du noyau : partiel.**

- **FEC** : `db::fec` sait produire le fichier 18 colonnes, et
  `llx_accounting_bookkeeping` reçoit désormais les écritures (VE/AC).
- **Pas d'archivage horodaté ni scellé** : rien ne prouve qu'un FEC produit
  aujourd'hui est celui du mois dernier.
- **Pas de conservation automatique** : les PDF vivent dans le dossier
  documents, sans politique de rétention.

---

## 5. Mentions obligatoires sur une facture

**Ce que dit la loi** : art. L441-9 du code de commerce, art. 242 nonies A de
l'annexe II au CGI. Numéro chronologique, dates, identités et adresses,
**SIREN/SIRET**, **numéro de TVA intracommunautaire**, décompte de la TVA par
taux, conditions et délais de règlement, **pénalités de retard**, montant de
l'indemnité forfaitaire de recouvrement (40 €), mention de l'acompte le cas
échéant.

**État du noyau : bon.** Le PDF porte CGV, RIB, mentions légales, ventilation
par taux, dates françaises, unités, « montants exprimés en euros », TVA sur
encaissements. C'est le point le plus solide du noyau — et il est testé.

---

## 6. Signature électronique d'un devis (eIDAS)

**Ce que dit la loi** : règlement (UE) 910/2014. Une signature « simple » a une
valeur probante faible ; une signature **avancée** ou **qualifiée** est
nécessaire pour un engagement ferme.

**État du noyau : absent.** Aucun lien de signature : le devis se valide par une
action interne, sans preuve d'accord du client côté noyau (ton workflow n8n le
faisait par un lien externe).

---

## 7. Site web et CNIL

Si le noyau expose un site public : **cookies** (consentement, exceptions
strictement nécessaires), **informations légales** (mentions, politique de
confidentialité), **accessibilité** (RGAA 4.1 pour le secteur public ; la
directive européenne 2019/882, applicable depuis le 28 juin 2025, vise les
produits et services grand public — un ERP B2B n'y est pas directement soumis,
le site vitrine oui).

**État du noyau : aucun site public.** Il expose une interface d'administration.

---

## 8. Paiement par carte (PCI-DSS)

Si le noyau encaisse par carte : **PCI-DSS** s'applique dès qu'un numéro de
carte transite. La règle de l'art : **ne jamais stocker ni voir le numéro**,
passer par une page hébergée par le prestataire.

**État du noyau : conforme par construction** — aucun champ de carte n'existe.

---

## Tableau de synthèse

| Obligation | État | Effort |
|---|---|---|
| Facturation électronique 2026/2027 | **absent** | semaines + partenariat PA |
| Logiciel de caisse (286 CGI) | **non conforme** | chantier (hachage, journal, verrou) |
| RGPD — sécurité | bon | — |
| RGPD — effacement, registre, durées | partiel/absent | jours (décisions + code) |
| FEC | **fait** | — |
| Archivage scellé 10 ans | absent | jours |
| Mentions de facture | **bon** | — |
| Signature électronique du devis | absent | jours (prestataire eIDAS) |
| Cookies / mentions du site | sans objet | — |
| PCI-DSS | conforme par construction | — |

## Ce que je ferais, dans l'ordre

1. **Ne pas présenter le noyau comme une caisse** tant que le point 2 n'est pas
   fait. C'est le seul écart qui expose à une sanction directe.
2. **L'archivage scellé** (point 4) : jours, pas semaines, et il protège tout le
   reste.
3. **La facture électronique** (point 1) : le vrai chantier, à mener avec un
   partenaire plutôt qu'en solitaire.
4. **Le registre RGPD** : un document, à écrire une fois.

## Sources consultées le 4 novembre 2026

- `entreprendre.service-public.gouv.fr/actualites/A15683` — calendrier de la
  facturation électronique
- `economie.gouv.fr` — certification des logiciels de caisse (page du
  24/02/2026, loi de finances pour 2026)
- `bofip.impots.gouv.fr` `ACTU-2025-00160` — certificat obligatoire, délai
  prorogé au 31 août 2026
- Textes cités de mémoire, non revérifiés ici : art. 286-I-3° bis CGI,
  art. L441-9 C. com., art. 242 nonies A ann. II CGI, art. L123-22 C. com.,
  art. A47 A-1 LPF, règlement (UE) 2016/679, règlement (UE) 910/2014,
  directive (UE) 2019/882.

---

## 9. Ce que l'open source change — et ce qu'il ne change pas

L'idée revient souvent : un projet libre serait « conforme par nature », ou le
serait plus facilement. C'est faux sur le plan juridique, et vrai sur un point
précis. La distinction décide de ce qu'on peut promettre.

### Ce que le libre ne change pas

| Obligation | Ce que le libre y change |
|---|---|
| Facturation électronique 2026 | **rien** — la réception par plateforme agréée est due, libre ou pas |
| Logiciel de caisse (286 CGI) | **rien sur le fond** — les quatre propriétés sont exigées du logiciel, pas de sa licence |
| RGPD | **rien** — les obligations pèsent sur le responsable et le sous-traitant |
| FEC, archivage 10 ans | **rien** |
| Mentions de facture | **rien** |
| Signature électronique | **rien** |

Publier le code sous GPL ne rend conforme à rien. Un logiciel libre peut être
hors cadre exactement comme un logiciel propriétaire.

### Ce que le libre change vraiment

1. **La preuve est vérifiable.** Un organisme accrédité peut auditer le code au
   lieu de croire une déclaration. Pour l'article 286 du CGI, c'est un avantage
   réel : l'inaltérabilité d'une chaîne de hachage se lit dans le source.
2. **L'attestation individuelle est possible.** La loi de finances pour 2026
   (adoptée le 19/02/2026) rétablit l'attestation délivrée par l'éditeur. En
   open source, l'éditeur **c'est toi** : tu signes, et tu engages ta
   responsabilité sur les quatre propriétés. Ce n'est pas une formalité.
3. **Le coût se partage.** L'archivage scellé, le format Factur-X, le connecteur
   de plateforme : écrits une fois, ils servent à tous les utilisateurs. C'est
   ainsi que Dolibarr tient — le cœur fournit la mécanique, l'utilisateur
   branche sa plateforme agréée.
4. **Le client reste libre de son prestataire.** Un ERP libre ne peut pas
   verrouiller la facturation électronique : il doit exposer des formats
   standards (Factur-X, UBL, CII) et laisser choisir la plateforme.

### La conséquence pratique

Le chemin réaliste n'est pas « devenir conforme en publiant ». C'est :

- **exposer Factur-X** pour que chaque utilisateur passe par sa plateforme ;
- **écrire la mécanique d'inaltérabilité** de la caisse — chaîne de hachage,
  journal append-only, verrou de suppression, export d'archivage — et la
  publier pour qu'elle soit auditable ;
- **ne pas signer d'attestation 286 CGI** tant que ces quatre propriétés ne sont
  pas implémentées et testées. Une attestation fausse engage l'éditeur.

### La phrase à pouvoir tenir

> « Le noyau est libre, il produit des factures au format Factur-X et il
> journalise les ventes de caisse de façon inaltérable ; la plateforme agréée et
> l'attestation de caisse relèvent de l'utilisateur et de son éditeur. »

Tant que la première moitié est fausse, la seconde ne protège de rien.
