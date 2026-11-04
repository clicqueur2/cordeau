# Plateformes agréées : ce qu'il faut savoir pour brancher le noyau

Recherche du **4 novembre 2026**. Sources vérifiées en bas. Les listes bougent :
celle-ci est datée, pas éternelle.

## Le cadre, en trois lignes

Depuis le **1er septembre 2026**, toute entreprise assujettie à la TVA établie en
France doit pouvoir **recevoir** des factures électroniques. L'**émission** suit :
grandes entreprises et ETI depuis 2026, **toutes les entreprises au 1er septembre
2027**.

Une facture électronique n'est pas un PDF par mail : c'est un **socle de données
structuré**, échangé **par l'intermédiaire d'une plateforme agréée** (PA, ex-PDP).
L'État ne transporte pas les factures ; il tient l'**annuaire** et reçoit le
**e-reporting**.

## PA, PPF, SC : qui fait quoi

| | **PA** (plateforme agréée, ex-PDP) | **PPF** (portail public) | **SC** (solution compatible) |
|---|---|---|---|
| Transport des factures | **oui** | non | non |
| Conversion de formats | oui | non | parfois |
| Annuaire des entreprises | non | **oui** | non |
| Suffit seule pour la réforme | **oui** | non | non (il faut une PA derrière) |

Notre noyau est un **SC** : il produit la facture au bon format. Il ne peut pas
remplacer une PA.

## Ce qu'une PA exige de notre côté

1. **Un format** : Factur-X (PDF + XML), UBL, ou CII. **Nous produisons le CII et
   le PDF porteur.**
2. **Un contrôle de qualité** des données : les règles EN 16931 que le noyau
   vérifie déjà (BR-CO-10/14/15, BR-DEC, BR-CL-03, BT obligatoires).
3. **Un adressage** : l'annuaire donne l'adresse de facturation du client (SIREN
   + code routage). C'est l'annuaire qui décide où va la facture, pas nous.
4. **Une authentification** : chaque PA fournit une API avec clé ou OAuth.

## Où trouver la liste officielle

```
Liste des PA (impots.gouv.fr)   https://www.impots.gouv.fr/je-consulte-la-liste-des-plateformes-agreees
Annuaire (état)                 https://facturation.chorus-pro.gouv.fr/annuaire/#/
```

Au **4 novembre 2026**, la page de la DGFiP est à jour du **3 novembre 2026**.
Les décomptes publiés par des tiers divergent (101, 145 selon les pages) parce
qu'ils comptent différemment les immatriculations « sous réserve ». **Seule la
page impots.gouv.fr fait foi** — et elle n'affiche pas de total.

## Celles que tu as citées, et ce qu'elles sont

| Plateforme | Statut | Ce qu'elle apporte |
|---|---|---|
| **Pennylane** | PA immatriculée | comptabilité + facturation + API ; importe du Factur-X et des XML |
| **Indy** | PA immatriculée | comptabilité pour indépendants |
| **Sellsy** | PA immatriculée | CRM + facturation |
| **Qonto** | PA immatriculée | banque + facturation |
| **Tiime** | PA immatriculée | comptabilité |
| **Cegid, Sage (via plateformes listées), Cegid, Esker, Basware, Tradeshift** | PA | grands comptes |

Les autres noms de la liste (ADEMICO, AGENA, ARTEVA, AVALARA, AXONAUT, B2BRouter,
Dougs, jefacture, Kollecto, Super PDP, Tiime, Voxel, Yooz…) sont soit des
plateformes pures, soit des éditeurs qui ont fait immatriculer leur offre.

## Trois stratégies possibles pour le noyau

**A — SC pur : produire le fichier, l'utilisateur l'envoie.**
Le noyau génère le CII ou le Factur-X, l'utilisateur dépose chez sa PA (ou lui
envoie par API depuis son espace). **Zéro intégration**, aucune dépendance à un
fournisseur, et c'est déjà utilisable. Coût : zéro.

**B — connecteur par PA : une API par plateforme.**
Chaque PA expose sa propre API (REST + clé ou OAuth 2). Il faut un module par
plateforme, et le maintenir. Coût : une semaine par PA, sans fin.

**C — passer par un agrégateur.**
Certains acteurs (B2BRouter, Super PDP, Tiime) revendent l'accès à plusieurs
plateformes derrière une API unique. Coût : un abonnement, une intégration.

Mon avis, et il ne change pas depuis trois jours : **A d'abord**. Le jour où un
utilisateur te dira « je suis chez Pennylane, peux-tu pousser directement ? »,
alors B ou C se justifiera — avec son API sous les yeux. Écrire un connecteur
pour une plateforme qu'on n'a pas encore vue, c'est travailler à l'aveugle.

## Ce que ça change dans `CONFORMITE-LEGALE-ERP.md`

| Obligation | Avant | Maintenant |
|---|---|---|
| Format Factur-X | absent | **CII produit, PDF porteur** |
| Règles EN 16931 | non vérifiées | **vérifiées** |
| Plateforme agréée | absente | **à choisir — c'est une décision, pas du code** |
| PDF/A-3 strict | absent | absent (voir note) |

## Le point sur PDF/A-3, dit simplement

Une PA sérieuse accepte **du XML seul** (CII ou UBL). Le PDF/A-3 est une
commodité pour la lecture humaine et pour l'archivage, pas une condition
d'entrée chez la plupart.

Donc : le noyau est **prêt à être branché**. Ce qui manque n'est pas du code,
c'est un **compte** chez une PA et sa clé d'API.

## Sources consultées le 4 novembre 2026

- `impots.gouv.fr/je-consulte-la-liste-des-plateformes-agreees` — page modifiée
  le 03/11/2026
- `impots.gouv.fr/facturation-electronique-et-plateformes-agreees` — rôle des PA,
  e-reporting, procédure d'immatriculation
- `economie.gouv.fr` — ouverture de l'annuaire, « près de 80 plateformes
  raccordées »
- `pennylane.com/fr/fiches-pratiques/facture-electronique/liste-des-pdp` —
  liste commentée (août 2026), à recouper avec impots.gouv.fr
- `indy.fr` — liste nominative des PA immatriculées (2026)

> Les listes de tiers divergent. Ne pas recopier leurs totaux : seule la page
> de la DGFiP fait foi, et elle ne publie pas de décompte stable.
