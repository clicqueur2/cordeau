# Guide d'utilisation

Ce logiciel est un noyau d'ERP en Rust qui lit et écrit une base **Dolibarr 24.0.1**. Ce n'est pas une copie : c'est la même base, avec ses 410 tables, ses comptes, ses droits et ses entités. Un devis créé ici se lit dans Dolibarr, et l'inverse est vrai.

Trois conséquences pratiques, à garder en tête pour tout le reste du guide.

La **société** (l'« entité », chez Dolibarr) est le cadre de tout. Un tiers, un devis, une facture, un compte bancaire appartiennent à une entité et ne se voient que dans cette entité.

Les **modules** décident de ce que la barre du haut affiche. Un module désactivé disparaît de la navigation ; ses données restent en base.

L'**interface est en français** et ne demande aucun JavaScript : chaque geste est un formulaire qui part au serveur et revient sur une page. C'est plus lent qu'une application moderne, mais rien ne se passe dans votre dos.

Dans tout ce guide, un chemin de menu est donné avec son adresse réelle entre parenthèses, par exemple : Outils → Société ou organisation (`/parametres`). Les encadrés « Refusé » citent le message que le logiciel renvoie quand il refuse quelque chose ; certains viennent des couches profondes du noyau et sont écrits sans accents, d'autres de l'interface et accentués — c'est le même message, à la typographie près.

---

## 1. Premiers pas

### Démarrer et installer

Le serveur écoute par défaut sur `http://127.0.0.1:8090` (variable `DOLIBARR_WEB_ADDR`), et se connecte à la base par `DOLIBARR_DSN`. Le binaire s'appelle `cordeau-web` : `cargo run -p web --bin cordeau-web`. La console affiche l'adresse retenue au démarrage, et c'est la première chose à lire si la page ne s'ouvre pas.

Le plus court reste le script : `.\demarrer.ps1` lance l'interface sur la copie de production (`-Base dolibarr` pour la base de test, `-Port 8091` si le port est déjà pris). On entre avec **son compte Dolibarr habituel** : la copie porte les vraies empreintes, et c'est `pass_crypted` que lit la connexion.

Au premier démarrage, ouvrez `/installation`. Cette page ne répare rien : elle constate. Elle affiche le nombre de tables présentes sur les tables attendues, la présence d'un administrateur, le nombre d'utilisateurs, les modules semés, le nom de la société, l'origine de la barre de menus et l'état du serveur d'e-mail. Tant qu'il n'y a aucun administrateur, elle propose de le créer : identifiant, mot de passe d'au moins huit caractères, e-mail, nom de la société. Le compte est créé avec le hachage bcrypt de Dolibarr, donc il fonctionnera aussi dans Dolibarr.

> **Refusé** — dès qu'un administrateur existe, `/installation` refuse de créer un compte, même si le formulaire est posté à la main. Sans cette garde, la page resterait une porte dérobée permettant de recréer un accès administrateur sur une installation en service.

### Se connecter

`/connexion` demande un identifiant et un mot de passe : ceux de la base, pas ceux d'un annuaire interne. La session dure huit heures ; passé ce délai, toute page renvoie vers la connexion.

Chaque connexion réussie et chaque échec sont consignés dans le journal des événements, avec la date, l'adresse IP et le navigateur. L'échec enregistre l'identifiant tenté, **jamais le mot de passe**. C'est justement quand on cherche une intrusion qu'on a besoin de cette trace, et c'est pourquoi un échec est enregistré sans entité : il reste visible depuis toutes les sociétés.

### Choisir la société

Si la base contient plusieurs entités, un sélecteur apparaît dans le bandeau. Le changer ne recharge pas la page par magie : c'est un formulaire (`POST /entite`) qui réenregistre l'entité de votre session, puis revient au tableau de bord. Toutes les requêtes suivantes travaillent dans cette entité, et la barre de menus est relue pour elle — un module peut être actif dans une société et pas dans l'autre.

Si le sélecteur n'apparaît pas, c'est qu'il n'y a qu'une entité : le logiciel ne montre pas un choix unique.

### Lire le tableau de bord

`/` affiche l'exercice en cours avec six indicateurs : le facturé, l'encaissé, le reste à encaisser, le devisé, le taux de signature et le montant en retard.

Trois d'entre eux se lisent mal si on ne sait pas ce qu'ils comptent, et c'est le genre de détail qui fait douter d'un chiffre juste.

Le « facturé » ne compte que les factures **validées** : un brouillon n'est pas un chiffre d'affaires.

Le « devisé » additionne les devis encore en **brouillon** (statut 0), pas les devis validés en attente de réponse. C'est ce que le code compte aujourd'hui.

Le taux de signature est le rapport entre les devis **signés** et les devis **tranchés** depuis le 1er janvier — signés plus perdus. Un devis encore ouvert ne tire donc pas le taux vers le bas, ce qui est la bonne façon de le lire.

En dessous, une barre par mois sur douze mois (ventes et achats), puis trois listes d'alerte : les factures validées non payées dont l'échéance est passée (huit au plus), les devis restés en brouillon depuis plus de quinze jours (six au plus), les produits sous leur seuil de stock — un seuil à zéro ne déclenche rien, sinon tout produit sans seuil alerterait en permanence. Le compteur « point(s) à traiter » additionne ces trois listes. Quand il vaut zéro, le logiciel le dit explicitement plutôt que d'afficher une page vide.

Viennent enfin les cinq meilleurs clients de l'année et les six dernières factures validées, avec leur reste dû et une pastille de statut.

### Chercher

La recherche du bandeau (ou Accueil → Recherche, `/recherche`) porte sur les tiers (nom, code client, e-mail), les produits (référence, libellé), les devis et les factures (référence **et nom du client**), les commandes (référence seulement) et les chantiers (titre, référence).

Chercher « Dupont » sort donc sa fiche, ses devis et ses factures. C'est volontaire : une recherche qui ne regarde qu'un champ par objet ne trouve que ce qu'on tape déjà. La recherche ne descend pas dans les lignes de documents : elle trouve des pièces, pas des prestations.

### Comprendre les droits

Un administrateur ouvre tout. Pour les autres comptes, chaque écran a un droit, et la lecture n'est pas l'écriture : consulter la liste des factures et valider une facture sont deux droits différents. Les droits se donnent par utilisateur (`/parametres/utilisateurs/<id>/droits`) ou par groupe (`/parametres/groupes`), et un compte cumule ce qu'il reçoit directement et ce que ses groupes lui donnent.

> **Refusé** — un écran sans droit affiche « Votre compte n'a pas le droit d'ouvrir cet écran », et la configuration (`/parametres`) renvoie vers Mon compte avec « Cet écran est réservé aux administrateurs ».

Pour retirer un droit hérité d'un groupe, il faut modifier le groupe : l'écran des droits d'un utilisateur le rappelle, parce qu'une case décochée ne retire pas ce que le groupe accorde.

### Le mode lecture seule

Si le serveur est démarré avec `DOLIBARR_LECTURE_SEULE`, toute méthode autre que la lecture est refusée avec « Mode lecture seule : aucune écriture n'est acceptée sur cette installation ». C'est le mode à utiliser pour brancher le noyau sur une copie d'une vraie base sans risque : on regarde, on n'abîme rien.

---

## 2. Un devis, de bout en bout

### Créer le devis

Commerce → Nouvelle proposition (`/devis/nouveau`). Vous choisissez le client (obligatoire), une note interne, la condition de règlement (lue dans le dictionnaire), un pourcentage d'acompte et une date de validité. Puis les lignes : désignation, quantité, prix unitaire hors taxes, taux de TVA.

L'écran de création accepte **quatre lignes**. C'est une limite de cet écran, pas du logiciel : pour un devis plus long, créez-le puis passez par « Modifier », qui en accepte dix et gère les sections.

> **Refusé** — un devis sans aucune ligne est refusé : « au moins une ligne est nécessaire ». Un devis vide ne veut rien dire et produirait un PDF vide.

La référence est attribuée à la création, au format `PR` + année-mois + compteur (par exemple `PR2611-0025`). Le devis naît en **brouillon**.

Les nombres se lisent à la française : la virgule est acceptée, `1 500,50` vaut 1500.50. Une saisie illisible n'est jamais interprétée comme zéro — un devis à zéro euro qui part chez un client serait pire qu'un message d'erreur.

### Modifier

Le bouton « Modifier » n'apparaît que sur un brouillon, et la modification refuse tout autre statut : « Seul un devis en brouillon peut être modifié ». La raison est simple : une pièce déjà validée a peut-être été envoyée au client, on ne la réécrit pas en silence.

L'écran d'édition remplace **toutes les lignes en bloc** : ce que vous ne remettez pas dans le formulaire disparaît. Une colonne « Section » ouvre un titre de lot : une ligne qui porte un nom de section sans désignation devient un intertitre, et les lignes suivantes s'y rattachent. C'est ce qui donne les devis par lots (dépose, support, repose, bordures) au lieu d'une liste plate.

### Valider

Sur la fiche du devis, « Valider le devis » passe le statut à validé, inscrit l'événement dans le journal (visible dans l'agenda et dans l'historique du tiers) et dépose le PDF dans la bibliothèque de documents.

> **Refusé** — un devis qui n'est pas en brouillon ne se valide pas, et un devis sans ligne non plus.

### Sortir le PDF

« Télécharger le PDF » (`/devis/<id>/pdf`) compose le document à la demande. L'en-tête porte la société, les mentions légales calculées (assurance décennale si elle est renseignée, TVA sur les encaissements, franchise en base, attestation de taux réduit sur facture, validité du devis) et les conditions générales saisies dans les paramètres. Le modèle de PDF se choisit dans Outils → Modèles de PDF : `standard` garde les annexes, `epure` et `compact` les retirent.

### Envoyer

Il n'y a pas d'écran « envoyer le devis ». Vous téléchargez le PDF et vous l'envoyez depuis votre messagerie. Le logiciel n'invente pas un envoi qu'il ne trace pas.

### La signature

C'est le point où il faut être précis, parce qu'il surprend. Le statut « signé » existe (c'est le statut 2 de Dolibarr), le filtre de la liste des devis le propose, mais **aucun bouton de l'interface ne fait passer un devis à signé**. Ce passage se fait par la ligne de commande du même logiciel :

```
cordeau proposal close --id 42 --statut 2
```

ou depuis Dolibarr lui-même, sur la même base, avec son module de signature.

Pourquoi cette absence ? Parce que Dolibarr n'ouvre les conversions qu'au statut signé, et que recopier une signature sans preuve (fichier, horodatage) serait pire que de ne rien faire. Tant que la signature n'est pas faite, le bouton « Facturer ce devis » est visible mais échoue — voir juste après.

### Transformer en commande

Le bouton « Créer la commande » apparaît sur la fiche du devis. Il exige un devis validé, et produit une commande client en brouillon, référence `CO` + année-mois + compteur, avec les mêmes lignes et la même condition de règlement. Le lien devis → commande est écrit dans la table des liens, ce qui permet de savoir plus tard quels devis ont été commandés.

> **Refusé** — « un devis non validé ne se commande pas ». Un brouillon doit d'abord être validé.

Sur la commande : « Valider la commande », puis « Facturer la commande ». Une commande sans ligne ne se valide pas, et une commande non validée ne se facture pas.

### Transformer en facture

Deux chemins, et ils ne se valent pas.

Depuis le devis, le bouton « Facturer ce devis » apparaît quel que soit le statut. S'il passe, il crée la facture et pose le lien devis → facture. Mais le noyau applique la règle de Dolibarr :

> **Refusé** — « devis PR… en statut 1 : seul un devis signé peut être facturé ». Il faut le statut 2 (signé) ou 4 (déjà facturé, reste convertible). Un même devis ne se facture qu'une fois : « ce devis a déjà été facturé (facture n° …) ».

Depuis la commande, « Facturer la commande » fait la même chose avec le lien commande → facture, et refuse aussi la double facturation.

### L'acompte

Le pourcentage d'acompte saisi sur le devis suit jusqu'à la commande. Sur la fiche de commande, si ce pourcentage est supérieur à zéro et qu'aucun acompte n'existe, un bouton apparaît : « Créer l'acompte (30 %) ». Il produit une facture d'acompte, une ligne par taux de TVA (sinon la TVA de l'acompte serait fausse), marquée `AC-<référence de commande>` pour empêcher le doublon.

> **Refusé** — « un acompte existe déjà pour cette commande (facture …) ». Un acompte calculé à zéro est refusé aussi.

Deux limites à connaître. L'acompte ne se crée pas depuis le devis, seulement depuis la commande. Et **l'acompte déjà facturé n'est pas déduit automatiquement de la facture finale** : c'est à vous de le déduire, ou d'attendre que ce soit codé.

### Facturer par situations

Sur la fiche d'un devis, le bloc « Facturation par situations » permet de facturer des tranches : un pourcentage et une date. Chaque situation crée une facture qui porte sa part, ventilée par taux de TVA.

La règle du cumul est stricte et c'est ce qui rend l'ensemble juste :

> **Refusé** — « le cumul dépasse 100 % : déjà 70 %, demande 40 % ». Un devis non validé ne se facture pas par situation.

Quand le cumul atteint 100 %, la dernière situation prend **le reste au centime** près, et non un pourcentage recalculé : la somme des situations égale alors le devis exactement. La fiche affiche le reste à facturer en pied de tableau.

Dès qu'un devis est facturé par situations, le logiciel désactive les boutons « Facturer ce devis » et « Créer la commande », avec le motif affiché : « Ce devis est facturé par situations : la facture globale est désactivée pour éviter la double facturation. »

---

## 3. Facturer

### La facture directe

Facturation → Facture directe (`/factures/nouveau`), pour ce qui n'a pas donné lieu à un devis. Client, bon de commande, condition de règlement, date de prestation, note interne, puis **six lignes** au maximum. La facture naît en brouillon.

Le numéro est attribué **à la création**, pas à la validation, comme le fait Dolibarr : `FA` + année-mois + compteur sur quatre chiffres. Le compteur ne repart pas à zéro au changement de mois : il continue, ce qui évite deux références identiques à cheval sur deux périodes. Un avoir sera préfixé `AV`, un acompte `AC`.

### Valider

« Valider la facture » passe le statut à validé, date la validation, inscrit l'événement et **dépose le PDF dans la bibliothèque de documents**.

> **Refusé** — une facture qui n'est plus en brouillon ne se valide pas : « la facture … n'est pas en brouillon ».

C'est la validation qui rend la facture encaissable. Avant, elle n'existe comptablement pas : elle n'entre ni dans le chiffre d'affaires, ni dans le grand livre, ni dans le FEC.

La liste des factures permet de cocher plusieurs brouillons et de les valider d'un geste. L'action de masse essaie **chaque ligne** et rend un bilan : les validées d'un côté, les refusées de l'autre avec leur motif. Elle ne s'arrête pas au premier refus, sinon vous ne sauriez pas ce qui est passé.

### Le PDF et le contrôle de conformité

Avant de composer une facture, le logiciel vérifie les **mentions légales obligatoires**. Si une information manque, il refuse de produire le document :

> **Refusé** — « facture non conforme : il manque adresse du vendeur, SIRET du vendeur » (ou le champ concerné).

Le contrôle porte sur les champs du vendeur (dénomination, adresse, code postal, ville, SIRET ou SIREN) et sur ceux du client : son nom, puis son SIRET et son numéro de TVA intracommunautaire s'il est professionnel. Un devis n'est pas soumis à ce contrôle : les mentions exigibles n'y sont pas les mêmes.

Conséquence pratique, et elle surprend : la validation archive le PDF, mais **si le PDF ne peut pas être produit, la validation passe quand même** et aucune pièce n'est archivée. Vous le découvrez au moment de l'envoi, avec « Aucun PDF dans la GED pour cette facture ». Le réflexe utile est donc de cliquer une fois sur « Télécharger le PDF » après la première validation d'une série : si le document sort, la chaîne est bonne ; sinon, le message dit exactement quel champ manque, dans Outils → Société ou organisation.

### La TVA à 0 %, et le motif d'exonération

La question revient souvent, alors voici exactement ce que le logiciel fait et ne fait pas.

Saisir une ligne à 0 % est **accepté sans autre formalité** : ni l'écran du devis, ni celui de la facture directe, ni celui de la facture fournisseur ne demande un motif d'exonération. Le logiciel ne bloque donc pas une facture à 0 % muette sur sa raison.

Ce qui est en revanche automatique, c'est la **mention légale** en pied de document, calculée à partir de trois réglages :

- si la société est en franchise en base, le PDF imprime « TVA non applicable, art. 293 B du CGI » ;
- si la société a coché « TVA sur les encaissements », le PDF imprime « TVA exigible sur les encaissements » ;
- si le client est professionnel (SIRET ou numéro de TVA renseigné) et que son pays n'est pas la France, le PDF imprime la mention d'autoliquidation.

La norme de facture électronique prévoit, elle, la règle, et le noyau la connaît : une ligne exonérée doit porter un code de motif (le code VATEX de la norme Factur-X) ou un texte de motif, faute de quoi la facture est déclarée non conforme ; et une ligne exonérée qui porterait un taux non nul se contredirait. Mais cette vérification n'est **pas encore branchée sur les écrans** : aujourd'hui, aucun champ ne demande le motif, et rien ne vous arrête. Autrement dit, la règle est écrite dans le code, elle n'est pas encore appliquée à la saisie. Tant que la facture électronique n'est pas émise, c'est à vous de veiller à ce que le 0 % soit justifié.

### Envoyer par e-mail

Sur une facture non brouillon, le bloc « Envoyer par e-mail » propose le destinataire (l'e-mail du tiers), un objet qui porte la référence, et un corps court. Vous pouvez choisir un modèle d'e-mail : ses variables sont alors remplacées, et une variable inconnue reste visible plutôt que de laisser un trou dans le message.

Trois refus, tous explicites, parce qu'une facture qui part sans sa pièce jointe n'est pas une facture envoyée :

> **Refusé** — « Adresse e-mail invalide : rien n'a été envoyé. » / « Aucun PDF dans la GED pour cette facture : validez-la d'abord. » / « Serveur SMTP non configuré : voir Paramètres > E-mails. »

La pièce jointe est le PDF **archivé**, pas un PDF recomposé au vol : ce qui part est exactement ce qui est conservé. L'envoi est tracé dans le journal des événements (`DOCUMENT_SENT`), et un échec SMTP laisse son motif dans le journal des erreurs.

### L'avoir

« Créer un avoir » n'apparaît que sur une facture validée qui n'est pas déjà un avoir. L'avoir reprend les lignes en quantités négatives, porte la référence `AV…`, et pointe sur la facture d'origine.

> **Refusé** — « un avoir ne peut pas recevoir d'avoir », « seule une facture validée peut être annulée par un avoir », « cette facture a déjà un avoir (n° …) ».

On ne supprime jamais une pièce comptable : on l'annule. C'est pour cette raison que le bouton de suppression n'existe pas sur une facture.

### La facture récurrente

Sur une facture non brouillon, le bloc « Facture récurrente » demande une périodicité (semaine, mois, trimestre, an), une prochaine échéance et un nombre (0 = sans fin). La série est une **copie figée** : modifier la facture d'origine ne change pas les suivantes, et c'est affiché sur l'écran.

Les échéances tombées se génèrent depuis `/recurrences` (écran présent, pas encore au menu). Le bouton « Générer les brouillons » produit des **brouillons**, jamais des factures validées ni des envois : vous relisez avant. Une clé `PA-RECUR-…` empêche le doublon si le traitement passe deux fois.

### Les devises

Facturation → Devises (`/devises`) sert à figer un taux **sur la pièce**. Une facture de mars garde le taux de mars, quoi que fasse le marché ensuite ; les montants de base restent en monnaie de la société, la devise n'est qu'une traduction. Le taux saisi est le nombre d'unités de devise pour 1 €.

Trois refus, et le troisième est le plus important :

> **Refusé** — « le taux doit être strictement positif » ; « devise inconnue ou inactive : … » ; « un taux est déjà figé sur cette pièce (…) : une pièce comptable ne se réévalue pas ».

Autrement dit, une facture validée garde son taux, même en forçant. C'est ce qui empêche un encaissement déjà comptabilisé de changer de montant six mois plus tard.

---

## 4. Encaisser

### Saisir un règlement

Le règlement se saisit depuis la fiche de la facture, dans le bloc « Enregistrer un règlement » : un montant (prérempli avec le reste dû), un mode (virement, chèque, espèces, carte), puis « Encaisser ».

Le logiciel refuse trois situations, et chaque refus protège une règle comptable :

> **Refusé** — « une facture en brouillon ne peut pas être réglée : validez-la d'abord » ; « une facture abandonnée ne peut pas être réglée » ; « la facture est déjà soldée ».

Encaisser sur un brouillon reviendrait à faire entrer de l'argent sur une pièce qui n'existe pas encore. La liste « Saisie des règlements » (`/reglements`) ne sert qu'à consulter : on n'y saisit rien, tout part de la facture concernée.

Chaque règlement écrit une **ligne bancaire** en même temps que l'affectation à la facture. Sans elle, Dolibarr ne verrait pas l'encaissement. Si vous ne précisez pas de compte, la ligne va sur le premier compte ouvert de l'entité ; s'il n'y en a aucun, la ligne bancaire n'est pas créée — le règlement existe, mais la banque l'ignore.

Quand le total des règlements atteint le TTC, la facture passe à « payée » automatiquement. Un règlement partiel laisse la facture ouverte avec un reste dû.

### Banques

Banques → Liste des comptes (`/banques`) affiche les comptes, leur IBAN et leur solde. Le solde **se déduit des écritures**, il n'est jamais stocké — c'est la même convention que Dolibarr, et c'est ce qui évite les soldes qui divergent du relevé. La page liste aussi les quarante dernières écritures avec un bouton pour les rapprocher.

Créer un compte demande une référence et un libellé (obligatoires), puis banque, IBAN et BIC.

> **Refusé** — « un compte porte déjà la référence … » : la référence est la clé du compte, deux comptes ne peuvent pas la partager.

### Rapprochement bancaire

Deux écrans, deux gestes différents, et il faut les distinguer.

« Pointer un encaissement » (`/banques/rapprochement`) liste ce qui est passé en banque sans être pointé, avec le montant, la facture et le tiers. Vous saisissez le numéro de relevé et vous pointez. **Pointer ne crée rien** : le paiement et sa ligne bancaire naissent ensemble à l'encaissement, le pointage marque seulement ce qui existe déjà. Le numéro de relevé est conservé pour retrouver, six mois plus tard, avec quel relevé la ligne a été pointée.

« Rapprochement bancaire » (l'ancre `/banques#rapprochement`) fait la même chose depuis la liste des dernières écritures, par bascule.

### Lettrage

Comptabilité → Lettrage (`/comptabilite/lettrage`) liste les comptes dont le solde n'est pas encore lettré, avec le montant et le nombre de lignes. Un bouton « Lettrer » apparie.

L'algorithme est **FIFO** : le débit le plus ancien mange le crédit le plus ancien. Ce n'est pas un rapprochement intelligent, et l'écran le dit : deux règlements du même montant se lettrent dans l'ordre des dates, pas selon la bonne facture. Vous obtenez ensuite le compte rendu exact : les paires rapprochées et ce qui reste ouvert.

Rejouer un lettrage ne défait pas le précédent : il ne travaille que sur le reste non lettré.

### Relances et impayés

Facturation → Relances et impayés (`/relances`) liste les factures échues et non réglées, avec le retard en jours et le reste dû. « Préparer le mail » ouvre votre messagerie avec un texte prérempli (référence, montant, retard). Sans adresse e-mail sur la fiche, la page affiche « pas d'e-mail » plutôt qu'un bouton mort.

Rien n'est envoyé automatiquement, et rien n'est marqué comme relancé : c'est un aide-mémoire, pas un robot.

---

## 5. Chantiers

Chantiers → Chantiers et tâches (`/chantiers`) liste les projets Dolibarr avec leur référence, leur client, leurs dates, leur budget hors taxes et leur état (ouvert ou clos).

Créer un chantier demande un intitulé ; le client est facultatif, et l'option « — chantier interne — » existe pour les travaux qui ne se facturent pas à un tiers. Vous pouvez saisir un début, une fin prévue, un budget HT et une description.

La fiche du chantier montre deux cartes (budget et nombre de tâches), la description, les tâches avec leurs dates et leur avancement. « Nouvelle tâche » ajoute un libellé, un début et une fin.

> **Refusé** — un chantier sans intitulé est refusé, et une tâche sans libellé aussi.

Deux choses ne sont pas encore là, et il vaut mieux le savoir avant de chercher.

La **marge du chantier** ne se lit pas sur la fiche : elle se calcule dans Comptabilité → Comptabilité analytique, après avoir créé un axe, puis un compte analytique par chantier, puis affecté les lignes d'écriture une par une (voir la section Comptabilité).

Le lien entre une facture et un chantier existe en base (`llx_facture.fk_projet`), mais **aucun écran ne le renseigne aujourd'hui**. La rubrique « Meilleurs chantiers » des statistiques reste donc vide tant que ce lien n'est pas posé ailleurs. Ce n'est pas une panne, c'est un chaînon manquant.

---

## 6. Achats

### Le fournisseur

Achats → Fournisseurs et factures (`/achats`) liste les fournisseurs (code, nom, ville, SIRET, e-mail), le total des achats TTC et les derniers paiements fournisseurs.

Créer un fournisseur demande un nom. Le code fournisseur est facultatif : laissé vide, il est généré.

> **Refusé** — « le nom du fournisseur est obligatoire », et « le code fournisseur … est déjà utilisé ».

### La facture fournisseur

« Saisir une facture fournisseur » (`/achats/facture`) demande le fournisseur, la référence chez le fournisseur, la date, une note interne, puis cinq lignes au maximum (désignation, quantité, prix unitaire HT, taux de TVA lu dans le dictionnaire). La facture est créée en brouillon, puis validée depuis la liste par le bouton « Valider ».

Trois refus méritent d'être connus.

> **Refusé** — « le fournisseur … a déjà une facture avec la référence fournisseur « … » ». Le couple fournisseur + référence est unique : c'est ce qui empêche de saisir deux fois la même facture reçue. Si vous laissez la référence vide alors qu'une facture sans référence existe déjà pour ce fournisseur, le message vous demande explicitement de renseigner la référence du document.

> **Refusé** — une date de facture illisible est refusée à la saisie (« date de facture invalide : … format attendu AAAA-MM-JJ ») plutôt que par une erreur SQL au milieu de l'enregistrement.

> **Refusé** — une facture fournisseur sans ligne ne se valide pas, et une facture déjà validée ne se revalide pas.

La date de facture fixe l'année-mois de la référence. C'est pour cela qu'elle est vérifiée avant tout le reste.

### Payer par virement SEPA

L'écran `/achats/sepa` (présent, pas encore au menu) liste les factures fournisseurs à payer avec leur échéance, leur IBAN et le reste à payer, puis produit un fichier `pain.001.001.03` que la banque sait lire.

Un avertissement s'affiche quand des IBAN manquent : « la banque rejetterait le lot entier ». Le fichier ne contient que les factures dont l'IBAN est connu — les autres sont silencieusement écartées du virement, ce qui est préférable à un rejet global, mais impose de lire l'avertissement avant de télécharger.

### Ce qui est en lecture seule

Les commandes fournisseurs (`/commandes-fournisseurs`), les réceptions (`/liste/Reception`) et les fiches d'intervention (`/liste/Ficheinter`) sont affichées depuis les tables Dolibarr sans écriture possible. Elles existent pour ne pas cacher des données réelles ; les écrans de saisie correspondants ne sont pas encore écrits.

---

## 7. Comptabilité

Un avertissement, valable pour toute cette section : le noyau produit des **écritures dérivées des documents** selon un plan comptable général simplifié. Ce n'est pas une comptabilité tenue par un expert-comptable, c'est une proposition à valider. L'écran du grand livre le dit lui-même.

### La synthèse

Comptabilité → Synthèse et TVA (`/comptabilite`) donne le chiffre d'affaires HT, les achats HT, la TVA collectée, la TVA déductible et le solde de TVA. Ce solde est **indicatif** et porte la mention « à vérifier avant déclaration » : il se calcule sur les factures, pas sur une déclaration.

Trois exports CSV datés sont proposés ici : journal des ventes, journal des achats, TVA par taux. Ils sont écrits pour un cabinet : point-virgule, virgule décimale, dates au format français, BOM UTF-8 pour qu'Excel ne massacre pas les accents.

### Les journaux

« Journal des ventes » et « Journal des achats » (menu Comptabilité) listent les pièces d'une période avec HT, TVA et TTC, et les totaux en pied. Ce sont les mêmes données que les exports CSV, à l'écran.

### Le grand livre et la balance

`/comptabilite/grand-livre` affiche une balance par compte (débit, crédit, solde) et le détail des écritures, filtrable par dates. C'est du **dérivé** : recalculé à partir des documents, à chaque affichage.

### Le livre tenu

`/comptabilite/livre-tenu` montre l'autre face : les lignes **réellement écrites** dans `llx_accounting_bookkeeping`, celles que Dolibarr lirait. Il affiche les 500 dernières, une balance par compte, et surtout un contrôle d'équilibre (« équilibre : oui / NON », avec « le livre ne tient pas » quand débit et crédit diffèrent). C'est l'écran à comparer avec le grand livre dérivé quand un chiffre vous paraît faux.

### Le FEC

Comptabilité → Comptabilisation et FEC (`/comptabilite/fec`).

« Générer les écritures » écrit les journaux VE (ventes), AC (achats) et BQ (banque) pour la période demandée. Cette génération **remplace** les écritures de ces trois journaux sur la période : elle est idempotente, mais elle écrase une correction manuelle faite dans ces journaux. En revanche, une écriture saisie dans un autre journal n'est jamais touchée, et c'est écrit sur l'écran.

Un brouillon ne s'écrit pas : la génération ne prend que les pièces valides.

« Exporter le FEC » produit le fichier au format de l'article A47 A-1 du livre des procédures fiscales. Sans dates, l'export prend tout (du 1er janvier 1970 au 31 décembre 2099).

### La clôture

`/comptabilite/cloture` ne clôt pas tout de suite : il **montre d'abord ce qu'il ferait**. Charges (classe 6), produits (classe 7), résultat, et les lignes exactes qui seraient écrites, compte par compte. Puis le bouton « Clore l'exercice ».

Le résultat est porté au compte 12 « Résultat de l'exercice ». Un instantané de l'exercice est déposé au moment de la fermeture : c'est la photographie de l'exercice, qui répondra plus tard sans recalculer sur un livre qui aura bougé. La clôture est aussi consignée dans le journal des événements, parce qu'elle change le résultat de la société.

> **Refusé** — « l'exercice … est déjà clos ». Le bouton devient alors « Rejouer la clôture », qui réécrit la même écriture sous le même numéro : le livre ne double pas, mais l'opération laisse une trace. Si rien n'a été écrit sur la période, l'écran affiche « Rien à solder » et ne propose pas de clôture.

### La comptabilité analytique

`/comptabilite/analytique` se travaille en trois temps.

1. Créer un axe (« CHANTIERS », « Chantiers »).
2. Créer des comptes analytiques rattachés à cet axe : un par chantier, avec un code (`CH-2026-014`) et un libellé.
3. Affecter une ligne d'écriture à un ou plusieurs comptes, au format `compte:pourcentage`, plusieurs parts séparées par des virgules.

> **Refusé** — « les pourcentages doivent valoir 100, pas 70 », « le compte … apparaît deux fois », « une part doit être strictement positive », « compte analytique … introuvable ».

L'affectation se fait **ligne à ligne, par numéro de ligne** dans `llx_accounting_bookkeeping`. C'est technique, et c'est assumé : il n'y a pas encore d'affectation automatique par chantier.

Le chiffre le plus important de cet écran n'est pas la marge, c'est le **taux d'affectation**. Une marge calculée sur 10 % des lignes n'est pas une marge, c'est un échantillon. Le taux est affiché à côté, en alerte dès qu'il passe sous 80 %.

### Écritures et immobilisations

Outils de comptabilité : « Écritures comptables » (`/liste/Accounting`) et « Immobilisations » (`/liste/Asset`) sont des listes génériques lisant les tables Dolibarr. « Nouvelle immobilisation » (`/liste/Asset/nouveau`) permet de créer une fiche : référence, libellé, date d'acquisition, début d'amortissement, valeur HT, quantité.

---

## 8. Stock et besoins

### Le catalogue

Produits / Services → Catalogue (`/produits`) liste les références avec leur prix HT, leur type (produit ou service) et leur unité.

Créer un produit (`/produits/nouveau`) demande une référence et un libellé, puis un prix HT, un taux de TVA, une unité (lue dans le dictionnaire) et une nature (produit ou service). La fiche produit (`/produits/<id>`) permet de modifier la désignation, le prix, la TVA, la description et la mise en vente. Elle rappelle que **la référence ne change jamais** : elle sert de clé dans les documents déjà émis. Un produit masqué (« En vente : non ») disparaît de la caisse.

### Le stock

Produits / Services → Stock (`/stock`). Le niveau affiché est la **somme des mouvements**, jamais une valeur saisie à part : c'est ce qui garantit qu'un inventaire et un historique racontent la même chose.

Avant tout mouvement, il faut un entrepôt. Si aucun n'existe, l'écran propose de le créer (référence obligatoire, lieu facultatif). Ensuite, un mouvement demande le produit, l'entrepôt, la quantité, le sens (entrée ou sortie) et un libellé.

> **Refusé** — « produit obligatoire », « entrepôt obligatoire », « quantité invalide » : un mouvement incomplet n'est pas enregistré.

Les cinquante derniers mouvements sont affichés avec leur date, leur sens et leur libellé.

### Les besoins et ordres proposés

Produits / Services → Besoins et ordres proposés (`/mrp`).

Le calcul part de la **demande** : les lignes de commandes clients validées et non expédiées. Le besoin net est la demande moins le stock — ce qui est en stock ne s'achète pas deux fois. Vous saisissez trois hypothèses : le lot d'achat, le minimum et le délai en jours, plus une date de besoin. L'écran rend, par article, la demande, le stock, le besoin net, la quantité proposée et la date de lancement, en rouge quand cette date est déjà passée.

Ce que le calcul **ne déduit pas**, et l'écran le dit : les entrées attendues (commandes fournisseurs en cours, ordres de fabrication) sont comptées comme nulles. Le besoin net est donc majoré d'autant. Le lot, le minimum et le délai sont des hypothèses que vous saisissez ici, parce que la fiche produit du schéma du cœur ne les porte pas.

### La caisse

Produits / Services → Caisse (`/caisse`) sert au comptoir : catalogue à gauche, panier, encaissement.

Deux chemins. Le panier : « + Ajouter au panier » sur un article, puis « Encaisser le panier ». La vente directe : un seul formulaire — article, quantité, moyen de paiement, montant reçu — qui envoie la vente en une ligne sans toucher au panier.

Le montant reçu est facultatif : vide, il vaut le total TTC. Plus élevé, l'écran affiche le rendu. Plus bas, la vente passe et **la facture reste en brouillon** : c'est une vente à crédit, et c'est dit dans le message.

> **Refusé** — « le panier est vide : ajoutez un article avant d'encaisser », « le montant encaissé doit être supérieur à zéro », « choisissez un moyen de paiement ».

Dolibarr n'a pas de facture sans client, donc la caisse nomme son client de passage : la constante `DOLIBARR_RS_TIERS_COMPTOIR` si elle est posée, sinon un tiers dont le nom évoque un client de passage (« comptoir », « anonyme », « divers »), sinon la création d'un « Client comptoir » à la première vente. L'écran ne crée jamais de donnée à l'affichage : seule une vente réelle en écrit une.

Trois limites à connaître. Le panier vit **en mémoire du serveur, par entité** : deux postes de caisse de la même société partageraient le même panier, et un redémarrage le vide (la vente encaissée, elle, est en base). Il n'y a pas de bouton pour décrémenter une ligne : on vide et on recommence. Enfin, aucun ticket n'est imprimé.

La conformité de caisse est traitée : la carte « Scellé du jour » calcule une empreinte des ventes du jour et permet de sceller la journée. Ensuite, l'état est « intact » (avec les douze premiers caractères de l'empreinte et un lien vers l'archive téléchargeable) ou **« FALSIFIÉ »** si les ventes ne correspondent plus au scellé — une ligne a été modifiée, ajoutée ou supprimée. L'archive est un fichier texte daté, à conserver : c'est la réponse à l'exigence d'inaltérabilité, de conservation et d'archivage des données de caisse.

### Lots, expéditions, réceptions

« Lots et séries » (`/liste/ProductBatch`) liste les lots avec leurs dates de consommation et de vente ; « Nouveau lot » permet d'en créer un (produit, numéro de lot, dates). « Expéditions » (`/expeditions`) affiche les bons de livraison avec leur état (brouillon, validé, livré). Ces écrans sont en lecture, sauf la création de lot.

---

## 9. Pilotage

### Statistiques

Comptabilité → Statistiques (`/statistiques`), sur 3 à 36 mois.

Quatre cartes : ventes, achats, marge brute (ventes moins achats) et encours client. Une précision qui compte : le chiffre d'affaires est **net**, les avoirs sont déduits, et une facture en brouillon ne compte pas. Puis les ventes et achats par mois en barres, les huit meilleurs clients et les huit meilleurs chantiers.

### Échéancier

Comptabilité → Échéancier (`/echeancier`), sur 3 à 12 mois : ce qui doit rentrer et sortir, **à l'échéance des pièces et non à leur date**. Un client en retard apparaît donc sur le mois où il aurait dû payer, ce qui est exactement ce qu'on veut voir venir.

La dernière colonne, le cumul, donne la position de trésorerie à la fin du mois si tout est encaissé et payé à l'échéance. Ce n'est pas un solde bancaire : le cumul ignore ce qui était déjà en caisse.

### Les alertes du tableau de bord

Trois listes, trois seuils : factures validées non payées dont l'échéance est passée, devis restés en brouillon depuis plus de quinze jours, produits sous leur seuil. Elles se recalculent à chaque affichage ; il n'y a pas de tâche de fond qui envoie des alertes.

Les réglages « Alertes » (jours de retard, destinataire) existent dans Outils mais ne déclenchent encore aucun envoi automatique.

### Les journaux

Deux journaux différents, à ne pas confondre.

Outils → Journal des événements (`/parametres/evenements`) consigne les connexions, les échecs de connexion, les changements de module, les envois de documents et les clôtures, avec l'adresse IP. C'est la trace de qui a fait quoi.

Outils → Journal des erreurs (`/parametres/journal`) lit le fichier d'erreurs du serveur : date, chemin, message. C'est là qu'on regarde quand un envoi de mail a échoué ou qu'un PDF n'a pas pu être lu.

---

## 10. Configuration

### La société émettrice

Outils → Société ou organisation (`/parametres`). Tout ce qui est saisi ici alimente l'en-tête et les mentions de **tous** les documents : dénomination, forme juridique, capital, RCS, adresse, SIRET, TVA intracommunautaire, e-mail, IBAN et BIC, assurance décennale (assureur, numéro de police, couverture), conditions générales de vente, délai de paiement, régime de TVA.

Trois réglages ont un effet visible sur les PDF, et il faut savoir lequel. « TVA taux réduit (travaux) » imprime l'attestation 279-0 bis du preneur sur les factures. « TVA sur les encaissements » imprime la mention correspondante. « Franchise en base (art. 293 B) » imprime « TVA non applicable, art. 293 B du CGI ». Se tromper ici, c'est imprimer une mention fausse sur une pièce légale.

Deux aides au remplissage évitent de recopier une fiche : la recherche par SIRET (annuaire de l'État) et la recherche par adresse. Ce sont deux simples formulaires de lecture, sans JavaScript : ils préremplissent la fiche, vous validez ensuite.

Le bouton « Insérer un modèle de CGV » remplit le champ avec un texte de départ (validité, paiement, garanties, réclamations). Il reste modifiable : c'est un point de départ, pas une clause validée par un juriste.

### La base de données

Outils → Base de données (`/parametres/base`) affiche le serveur, l'hôte et le port, le nom de la base, le nombre de tables et de colonnes, et la version de schéma de référence. Deux contrôles utiles : la **conformité de la société** (les champs légaux manquants pour éditer une facture sont listés, en pastille rouge) et les modules Dolibarr activés.

Un « complète » en vert signifie que l'en-tête de facture a tout ce qu'il faut. Le contrôle porte sur les champs du **vendeur** — dénomination, adresse, code postal, ville, SIRET ou SIREN — parce que le client n'est ici qu'un rôle fictif : ce qui manque côté client se vérifie à l'émission, sur la vraie fiche.

### Les dictionnaires

Outils → Dictionnaires (`/parametres/dictionnaires`). Les données sont **lues directement dans la base Dolibarr**, jamais recopiées : ajouter une entrée dans Dolibarr la rend disponible ici.

Ce que vous pouvez faire depuis cet écran : créer une condition de règlement (code, libellé client, délai en jours, fin de mois) et en désactiver une. C'est ce libellé qui s'imprime sur les documents — un libellé mal écrit se retrouve donc sur toutes les factures.

> **Refusé** — une condition sans code ni libellé est refusée : « code de la condition obligatoire ».

Les taux de TVA, modes de paiement et unités sont affichés. Les taux se créent depuis Outils → Nouveau taux (`/liste/Tax/nouveau`) ; les modes de paiement et les unités ne se modifient pas ici et doivent être édités dans Dolibarr. C'est volontaire : ce sont des dictionnaires partagés, et une écriture à moitié outillée est pire qu'une absence d'écriture.

La page termine par le recensement de toutes les tables `llx_c_*` avec leur nombre de lignes : c'est le garde-fou qui montre qu'aucun dictionnaire n'est invisible.

### Les champs libres

Outils → Champs libres (`/parametres/extrafields`) liste les objets qui portent des champs libres et leur nombre. Dolibarr ajoute une **colonne** à la table de l'objet pour chaque champ (`options_<nom>`) et tient la définition dans sa table de définitions.

Trois niveaux : la liste des objets, les champs d'un objet (`/parametres/extrafields/<objet>`) avec leur type, leur colonne et leur état, puis les **valeurs** d'un objet précis (`/extrafields/<objet>/<numéro>`), modifiables.

Deux points à connaître. Les **définitions** se créent dans Dolibarr, pas ici : cet écran montre et saisit des valeurs. Et une colonne absente est signalée en rouge (« — absente ») : la définition existe mais la colonne manque en base, donc la valeur ne pourra pas s'enregistrer.

> **Refusé** — « champ inconnu ou non saisissable : … » : un champ désactivé ou calculé par expression ne s'écrit pas à la main. Et « colonne absente pour le champ … » : la définition existe, la colonne manque, rien ne peut être enregistré.

Les noms de champs sont ceux de Dolibarr (`options_<nom>`), donc ce que vous saisissez ici serait accepté par Dolibarr, et l'inverse est vrai.

### Les catégories

Outils → Catégories (`/categories`). Une catégorie appartient à une **famille**, et il y en a huit : produits et services, fournisseurs, clients, contacts, chantiers, commandes, factures, devis. L'écran impose la famille avant tout : une catégorie de clients ne peut pas étiqueter un produit. C'est la première chose qu'il montre, parce que c'est la première erreur qu'on ferait. La famille se choisit par le sélecteur en haut, et se retrouve dans l'adresse (`/categories?famille=facture`).

Vous créez une catégorie avec un libellé, une description et une couleur, puis vous étiquetez un objet en saisissant son **numéro** (le `rowid` de la ligne dans la table Dolibarr de cette famille). L'écran affiche ce nom de table pour que vous sachiez où regarder. « Retirer » défait l'étiquette.

> **Refusé** — « la catégorie … n'existe pas dans cette famille », « numéro d'objet invalide ».

### Les modèles d'e-mail

Outils → Modèles d'e-mail (`/parametres/modeles-emails`). Un modèle porte un type, un libellé, un objet et un corps. Les variables disponibles sont `__REF__`, `__TOTAL_TTC__`, `__TOTAL_HT__`, `__CLIENT__`, `__ECHEANCE__` et `__SOCIETE__`. Une variable inconnue **reste visible** dans le message plutôt que de laisser un trou : mieux vaut un `__TOTO__` dans un brouillon qu'une phrase amputée.

Un modèle peut être actif ou inactif ; les modèles actifs apparaissent dans le sélecteur de la fiche facture, avec un bouton « Appliquer le modèle ».

### Les campagnes d'e-mailing

`/campagnes` (écran présent, pas encore au menu) compose un message, compte les destinataires par cible (clients, prospects, fournisseurs, une fois par adresse) et envoie un **essai** à une seule adresse. Rien de plus, et c'est délibéré : tant que chaque envoi n'est pas tracé dans la table des mailings de Dolibarr, un envoi en masse produirait des courriers sans trace, impossibles à relancer comme à auditer. Le message de confirmation le dit sans détour : la campagne vers la cible n'est pas lancée, seul un message de contrôle est parti.

En attendant, l'écran « Emailings » (`/liste/Mailing`) montre les campagnes qui existent déjà en base, en lecture.

### Les modules

Outils → Modules / Applications (`/parametres/modules`). Le catalogue est classé par famille (base, commercial, financier, produits, projets, achats, RH, documents, portails, interfaces, technique). Chaque module porte son nom technique, son état, et la mention « porté » quand ses écrans existent réellement dans ce noyau.

Activer ou désactiver écrit la constante Dolibarr `MAIN_MODULE_<CODE>` : Dolibarr PHP lit le même état. L'opération est consignée dans le journal des événements, parce qu'un module qu'on active change ce que l'ERP sait faire. La barre de menus est rechargée immédiatement.

Un module actif qui a des options réglables affiche un bouton « Options » (`/parametres/module/<code>`) : chaque option est une constante Dolibarr, avec son aide et son nom technique.

### Les réglages

Outils → Affichage, Menus, Traduction, Valeurs/filtres/tris, Widgets, Alertes, Sécurité, Limites et précision, Modèles de PDF, E-mails et notifications, SMS, Divers.

Chaque écran est un formulaire de constantes : vous saisissez, le logiciel écrit dans les constantes de la société. Toutes ces valeurs ne sont pas encore lues partout, et il vaut mieux le dire que de laisser croire le contraire. Sont réellement appliqués aujourd'hui : le modèle de PDF (`standard`, `epure`, `compact`), les paramètres SMTP, et la clé d'API.

Les marges du gabarit PDF et le pied de page sont enregistrés mais **pas encore lus** par le moteur de rendu : les modifier ne changera pas vos PDF pour l'instant.

L'écran « E-mails et notifications » mérite un détour : il propose d'envoyer un **essai** qui part vraiment par le serveur configuré. Une erreur SMTP s'affiche telle quelle (« 535 5.7.8 Authentication failed »), ce qui dit quoi corriger. Un mot de passe SMTP peut aussi être fourni par la variable d'environnement `DOLIBARR_SMTP_PASSWORD`, qui l'emporte sur la base : un secret n'a pas à traîner dans une table quand le serveur peut le fournir.

L'écran « Sécurité » contient la clé d'API (en-tête `DOLAPIKEY`) et la longueur minimale des mots de passe. La clé est lue au démarrage du serveur : une clé modifiée ici prend effet au prochain redémarrage, pas avant.

### Utilisateurs, groupes, droits

Outils → Utilisateurs et groupes (`/parametres/utilisateurs`). Vous créez un compte avec un identifiant, un mot de passe d'au moins huit caractères, un prénom, un nom, un e-mail et un rôle. Les mots de passe créés ici fonctionnent aussi dans Dolibarr. Vous pouvez désactiver un compte sans le supprimer.

Les droits se cochent écran par écran, par utilisateur (`/parametres/utilisateurs/<id>/droits`). Les groupes (`/parametres/groupes`, accessible par URL ou depuis la page des droits) portent des droits que leurs membres héritent ; on y gère aussi les membres.

### Import et export

Outils → Import / Export (`/parametres/import`). L'import se fait par copier-coller de CSV, séparé par `;` ou `,`, avec une ligne d'en-tête : les colonnes sont reconnues par leur nom.

- Tiers : `nom` (obligatoire), `adresse`, `cp`, `ville`, `email`, `tel`, `siret`, `tva`.
- Produits : `ref` (obligatoire), `libelle`, `prix`, `tva`, `type` (produit ou service), `unite`.
- Factures : une ligne = une ligne de facture, regroupée par client et référence. Colonnes `client` (obligatoire), `facture`, `date`, `echeance`, `libelle`, `qte`, `pu`, `tva`.

L'import rend un rapport : créés, ignorés (déjà présents ou sans clé), erreurs listées une par une.

> **Refusé** — « colonne « nom » introuvable dans l'en-tête » : sans la colonne obligatoire, rien n'est importé.

Attention à une conséquence : **les factures importées sont créées puis validées automatiquement**, le client est créé s'il n'existe pas, et la numérotation Dolibarr leur est attribuée. Un import de factures n'est donc pas une répétition sans effet.

Les exports (tiers, produits, ventes, achats, TVA, grand livre en CSV ; FEC en `.txt`) s'obtiennent depuis les écrans correspondants.

### La sauvegarde

Outils → Sauvegarde (`/parametres/sauvegarde`) fait un dump SQL complet plus le dossier documentaire, dans un dossier horodaté. Le chemin racine se règle par `DOLIBARR_SAUVEGARDES`, et l'outil `mysqldump` doit être installé (variable `DOLIBARR_MYSQLDUMP`) ; sinon l'écran affiche « La sauvegarde a échoué ».

La restauration n'est **pas** dans l'interface, et c'est volontaire : elle se fait en ligne de commande, avec la commande affichée à l'écran. Une restauration déclenchée par un clic sur un serveur en production est un moyen trop simple de tout perdre.

---

## 11. Personnaliser sans coder

Quatre mécanismes, du plus simple au plus technique. Les trois premiers ne demandent pas de toucher au binaire.

### Les champs libres

C'est le premier recours, et souvent le seul nécessaire. Une définition créée dans Dolibarr ajoute une colonne à la table de l'objet, et l'écran `/parametres/extrafields` la rend visible et saisissable. Aucune compilation, aucun fichier à déposer.

Limite : on ajoute des champs à un objet existant, on ne crée pas un nouvel écran.

### Les modules déclaratifs (JSON)

Un fichier `.json` déposé dans le dossier `modules` (ou celui de `DOLIBARR_MODULES`) ajoute un module à la barre : un menu, un droit, des listes en lecture et des formulaires de saisie. Le fichier décrit ce que le module fait ; le noyau l'exécute.

```json
{
  "code": "Entretien",
  "libelle": "Contrats d'entretien",
  "menu": { "cle": "entretien",
            "entrees": [ { "cle": "liste", "libelle": "Contrats", "lien": "/liste/Entretien" } ] },
  "droit": { "id": 9501, "libelle": "Contrats d'entretien" },
  "listes": [ { "code": "Entretien", "titre": "Contrats", "intro": "…",
                "sql": "SELECT … WHERE entity = :entity", "colonnes": ["N°", "Réf.", "Client"] } ],
  "saisies": [ { "code": "Entretien", "table": "llx_entretien",
                 "champs": [ { "nom": "ref", "libelle": "Référence", "sorte": "texte", "obligatoire": true } ],
                 "fixes": [ ["datec", "NOW()"] ],
                 "reference": ["ref", "CTR"] } ]
}
```

Ce que le chargeur **refuse**, et pourquoi : le SQL est vérifié. Un seul `SELECT`, sans commentaire ni point-virgule, et `:entity` obligatoire pour qu'une liste ne puisse pas déborder d'une entité — sauf pour une table qui n'a pas de colonne d'entité (un dictionnaire national), signalée par `sans_entite`. Les noms de table et de colonne suivent la même règle que partout dans le noyau.

Un champ accepte quatre sortes : `texte`, `nombre`, `date` et `entier` ; une sorte inconnue fait refuser le fichier au chargement, ce qui vaut mieux qu'un formulaire qui enregistre du texte dans une colonne numérique. La clé `fixes` pose des valeurs constantes à l'insertion (ici la date de création), et `reference` demande une référence numérotée automatiquement avec son préfixe.

Un module déclaratif ne peut pas faire plus que ce que le noyau sait déjà faire. En échange, il ne touche pas au binaire.

### Les greffons (bibliothèques)

Outils → Greffons (`/parametres/greffons`) liste les bibliothèques chargées à chaud (`.dll` sous Windows, `.so` ailleurs) trouvées dans le dossier `greffons` (ou `DOLIBARR_GREFFONS`), avec le contrat ABI C attendu. C'est le mécanisme pour du code compilé, et il s'adresse à quelqu'un qui sait déjà écrire une bibliothèque C.

### L'API REST

Pour brancher un automate, un script ou un agent : `/api`, authentifiée par l'en-tête `DOLAPIKEY` (la clé se règle dans Outils → Sécurité ou par `DOLIBARR_API_KEY`). La clé vaut pour une entité, choisie par `DOLIBARR_API_ENTITY`.

Les routes disponibles sont volontairement peu nombreuses : `ping`, les tiers (lecture et création), les produits (lecture), les devis (lecture, création, validation, situation, commande), les factures (lecture, fiche, règlement), les commandes (lecture, validation, facturation) et les banques (lecture). Une route absente n'est pas un oubli de documentation : elle n'existe pas.

Les montants sortent en **chaînes décimales**, jamais en nombres flottants. Un automate qui arrondit un total est un automate qui casse une comptabilité.

---

## Ce que ce guide ne couvre pas

Ce guide décrit les écrans et les règles du noyau, pas l'exploitation d'un serveur. Il ne dit rien de l'installation de MySQL, de la sauvegarde système, du reverse proxy, des certificats, ni du durcissement du système — ce sont des sujets d'administration, traités dans les documents d'installation du dépôt.

Il ne couvre pas non plus les fonctions de Dolibarr qui n'ont pas encore d'écran ici : la signature client d'un devis, l'envoi de devis par e-mail, la déduction automatique d'un acompte sur la facture finale, l'affectation automatique d'un chantier à une facture, l'envoi de campagnes d'e-mailing à toute une cible, les notes de frais, les congés, les salaires, la paie et les prélèvements SEPA. Ces données sont visibles en lecture quand elles existent en base, mais les gestes correspondants ne sont pas encore écrits.

Enfin, il ne remplace ni un expert-comptable ni un juriste : les mentions légales, les taux de TVA et les écritures proposées doivent être vérifiés par ceux dont c'est le métier.

---

## En cinq lignes

Ce guide suit le parcours réel d'une entreprise dans le logiciel : se connecter et choisir sa société, monter un devis jusqu'à la commande et à la facture, encaisser et rapprocher, suivre un chantier, acheter, tenir les comptes jusqu'au FEC et à la clôture, gérer stock et besoins, piloter par les chiffres, puis configurer sans code.

Il documente les refus autant que les boutons : brouillon non encaissable, devis non signé non facturable, devis facturé deux fois impossible, cumul de situations plafonné à 100 %, écritures d'un brouillon inexistantes, facture sans mentions légales non imprimable.

Il distingue ce qui est appliqué de ce qui est seulement prévu : la règle du motif d'exonération à 0 % est écrite pour la facture électronique, mais aucun écran ne la demande encore.

Il ne traite pas l'exploitation du serveur, ni les modules Dolibarr restés sans écran, ni la conformité fiscale, qui demandent un professionnel.

Il est à relire à chaque version : plusieurs écrans existent déjà sans être au menu, et ce qui manque ici finira par y être.
