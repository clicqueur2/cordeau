# Écrans en erreur — balayage sur la copie de production

255 routes GET essayées, **29 en défaut**.

| Code | Route |
|---|---|
| 500 | `/achats/sepa/fichier` |
| 404 | `/api` |
| 500 | `/liste/:code` |
| 500 | `/liste/:code/nouveau` |
| 500 | `/parametres/module/:code` |
| 500 | `/parametres/reglage/:cle` |
| 500 | `/parametres/tables/:nom` |
| 500 | `/transferts` |
| 404 | `/achats/fournisseur/1/modifier` |
| 404 | `/commandes/1` |
| 404 | `/commandes/1/pdf` |
| 404 | `/connaissances/1` |
| 404 | `/dons/1` |
| 404 | `/expeditions/1` |
| 500 | `/extrafields/:elementtype/1` |
| 404 | `/immobilisations/1` |
| 404 | `/interventions/1` |
| 404 | `/inventaires/1` |
| 404 | `/mailings/1` |
| 404 | `/marges/1` |
| 404 | `/nomenclatures/1` |
| 404 | `/notes-frais/1` |
| 404 | `/parametres/groupes/1` |
| 404 | `/postes/1` |
| 404 | `/prets/1` |
| 404 | `/receptions/1` |
| 404 | `/sites/1` |
| 404 | `/tickets/1` |
| 500 | `/transferts/1` |

## Routes essayées

- `/` → 200
- `/achats` → 200
- `/achats-fournisseur` → 200
- `/achats-fournisseur/commande` → 405
- `/achats-fournisseur/demande` → 405
- `/achats/facture` → 200
- `/achats/fournisseur` → 405
- `/achats/sepa` → 200
- `/achats/sepa/fichier` → 500
- `/adherents` → 200
- `/agenda` → 200
- `/agenda/nouveau` → 405
- `/api` → 404
- `/avoirs` → 200
- `/banques` → 200
- `/banques/rapprochement` → 200
- `/banques/rapprochement/pointer` → 405
- `/banques/rapprocher` → 405
- `/caisse` → 200
- `/caisse/ajouter` → 405
- `/caisse/archive` → 200
- `/caisse/encaisser` → 405
- `/caisse/sceller` → 405
- `/caisse/vider` → 405
- `/campagnes` → 200
- `/campagnes/envoyer` → 405
- `/categories` → 200
- `/categories/affecter` → 405
- `/categories/creer` → 405
- `/categories/retirer` → 405
- `/chantiers` → 200
- `/commandes` → 200
- `/commandes-fournisseurs` → 200
- `/commandes/actions` → 405
- `/comptabilite` → 200
- `/comptabilite/analytique` → 200
- `/comptabilite/analytique/affecter` → 405
- `/comptabilite/analytique/axe` → 405
- `/comptabilite/analytique/compte` → 405
- `/comptabilite/cloture` → 200
- `/comptabilite/fec` → 200
- `/comptabilite/grand-livre` → 200
- `/comptabilite/journal` → 200
- `/comptabilite/lettrage` → 200
- `/comptabilite/lettrage/:compte` → 405
- `/comptabilite/livre-tenu` → 200
- `/compte` → 200
- `/conges` → 200
- `/connaissances` → 200
- `/connexion` → 200
- `/contacts` → 200
- `/contrats` → 200
- `/deconnexion` → 405
- `/devis` → 200
- `/devis/actions` → 405
- `/devis/nouveau` → 200
- `/devises` → 200
- `/devises/figer` → 405
- `/documents` → 200
- `/dons` → 200
- `/echeancier` → 200
- `/entite` → 405
- `/expeditions` → 200
- `/expeditions/depuis-commande` → 405
- `/export/achats.csv` → 200
- `/export/fec.txt` → 200
- `/export/grand-livre.csv` → 200
- `/export/produits.csv` → 200
- `/export/tiers.csv` → 200
- `/export/tva.csv` → 200
- `/export/ventes.csv` → 200
- `/factures` → 200
- `/factures/actions` → 405
- `/factures/nouveau` → 200
- `/immobilisations` → 200
- `/installation` → 200
- `/interventions` → 200
- `/interventions/nouveau` → 405
- `/inventaires` → 200
- `/liste/:code` → 500
- `/liste/:code/nouveau` → 500
- `/livraisons` → 200
- `/mailings` → 200
- `/marges` → 200
- `/mrp` → 200
- `/nomenclatures` → 200
- `/notes-frais` → 200
- `/parametres` → 200
- `/parametres/base` → 200
- `/parametres/cgv-modele` → 405
- `/parametres/conditions-reglement` → 405
- `/parametres/conditions-reglement/desactiver` → 405
- `/parametres/constantes` → 200
- `/parametres/dictionnaires` → 200
- `/parametres/emails/essai` → 405
- `/parametres/evenements` → 200
- `/parametres/extrafields` → 200
- `/parametres/extrafields/:elementtype` → 200
- `/parametres/greffons` → 200
- `/parametres/groupes` → 200
- `/parametres/import` → 200
- `/parametres/import/factures` → 405
- `/parametres/import/produits` → 405
- `/parametres/journal` → 200
- `/parametres/journaux` → 200
- `/parametres/modeles-emails` → 200
- `/parametres/modeles-emails/basculer` → 405
- `/parametres/modeles-emails/creer` → 405
- `/parametres/modeles-pdf` → 200
- `/parametres/module/:code` → 500
- `/parametres/modules` → 200
- `/parametres/notifications` → 200
- `/parametres/planification` → 200
- `/parametres/planification/desactiver` → 405
- `/parametres/reglage/:cle` → 500
- `/parametres/sauvegarde` → 200
- `/parametres/securite` → 200
- `/parametres/societe` → 405
- `/parametres/tables` → 200
- `/parametres/tables/:nom` → 500
- `/parametres/traduction` → 200
- `/parametres/utilisateurs` → 200
- `/parametres/utilisateurs/nouveau` → 405
- `/partenariats` → 200
- `/postes` → 200
- `/presets` → 200
- `/prets` → 200
- `/produits` → 200
- `/produits/nouveau` → 200
- `/receptions` → 200
- `/recherche` → 200
- `/recurrences` → 200
- `/recurrences/generer` → 405
- `/reglements` → 200
- `/relances` → 200
- `/sites` → 200
- `/statistiques` → 200
- `/stock` → 200
- `/stock/entrepot` → 405
- `/stock/mouvement` → 405
- `/tickets` → 200
- `/tickets/actions` → 405
- `/tiers` → 200
- `/tiers/nouveau` → 200
- `/transferts` → 500
- `/achats-fournisseur/demande/1/cloturer` → 405
- `/achats-fournisseur/demande/1/refuser` → 405
- `/achats-fournisseur/demande/1/valider` → 405
- `/achats/facture/1/valider` → 405
- `/achats/fournisseur/1/modifier` → 404
- `/adherents/1/cotiser` → 405
- `/adherents/1/reactiver` → 405
- `/adherents/1/suspendre` → 405
- `/banques/16/modifier` → 200
- `/chantiers/304` → 200
- `/chantiers/304/modifier` → 200
- `/chantiers/304/tache` → 405
- `/commandes/1` → 404
- `/commandes/1/acompte` → 405
- `/commandes/1/facturer` → 405
- `/commandes/1/pdf` → 404
- `/commandes/1/valider` → 405
- `/conges/1/annuler` → 405
- `/conges/1/approuver` → 405
- `/conges/1/refuser` → 405
- `/conges/1/valider` → 405
- `/connaissances/1` → 404
- `/connaissances/1/depublier` → 405
- `/connaissances/1/publier` → 405
- `/contacts/1399/modifier` → 200
- `/devis/639` → 200
- `/devis/639/commander` → 405
- `/devis/639/facturer` → 405
- `/devis/639/modifier` → 200
- `/devis/639/pdf` → 200
- `/devis/639/perdu` → 405
- `/devis/639/signer` → 405
- `/devis/639/situation` → 405
- `/devis/639/valider` → 405
- `/dons/1` → 404
- `/dons/1/encaisser` → 405
- `/dons/1/valider` → 405
- `/expeditions/1` → 404
- `/expeditions/1/expedier` → 405
- `/expeditions/1/supprimer` → 405
- `/expeditions/1/valider` → 405
- `/extrafields/:elementtype/1` → 500
- `/factures/714` → 200
- `/factures/714/avoir` → 405
- `/factures/714/envoyer` → 405
- `/factures/714/pdf` → 200
- `/factures/714/recurrence` → 405
- `/factures/714/reglement` → 405
- `/factures/714/valider` → 405
- `/immobilisations/1` → 404
- `/immobilisations/1/ceder` → 405
- `/interventions/1` → 404
- `/interventions/1/cloturer` → 405
- `/interventions/1/valider` → 405
- `/inventaires/1` → 404
- `/inventaires/1/valider` → 405
- `/liste/Pays/fiche/1` → 200
- `/liste/Pays/fiche/1/supprimer` → 405
- `/livraisons/1/livrer` → 405
- `/livraisons/1/valider` → 405
- `/mailings/1` → 404
- `/mailings/1/approuver` → 405
- `/mailings/1/marquer-envoye` → 405
- `/mailings/1/valider` → 405
- `/marges/1` → 404
- `/marges/1/ligne` → 405
- `/marges/1/valider` → 405
- `/nomenclatures/1` → 404
- `/nomenclatures/1/valider` → 405
- `/notes-frais/1` → 404
- `/notes-frais/1/approuver` → 405
- `/notes-frais/1/refuser` → 405
- `/notes-frais/1/soumettre` → 405
- `/pages/1/depublier` → 405
- `/pages/1/publier` → 405
- `/pages/1/supprimer` → 405
- `/parametres/groupes/1` → 404
- `/parametres/groupes/1/droits` → 405
- `/parametres/groupes/1/membres` → 405
- `/parametres/utilisateurs/1/basculer` → 405
- `/parametres/utilisateurs/1/droits` → 200
- `/partenariats/1/accepter` → 405
- `/partenariats/1/annuler` → 405
- `/partenariats/1/refuser` → 405
- `/partenariats/1/soumettre` → 405
- `/postes/1` → 404
- `/postes/1/desactiver` → 405
- `/postes/1/groupe` → 405
- `/postes/1/modifier` → 405
- `/postes/1/ressource` → 405
- `/postes/1/valider` → 405
- `/prets/1` → 404
- `/prets/1/regler` → 405
- `/produits/2239` → 200
- `/produits/2239/modifier` → 405
- `/receptions/1` → 404
- `/receptions/1/receptionner` → 405
- `/receptions/1/supprimer` → 405
- `/receptions/1/valider` → 405
- `/sites/1` → 404
- `/sites/1/pages` → 405
- `/sites/1/supprimer` → 405
- `/tickets/1` → 404
- `/tickets/1/fermer` → 405
- `/tickets/1/statut` → 405
- `/tiers/1272` → 200
- `/tiers/1272/modifier` → 200
- `/transferts/1` → 500
- `/transferts/1/expedier` → 405
- `/transferts/1/recevoir` → 405
