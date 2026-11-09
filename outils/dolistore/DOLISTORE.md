# Dolistore : ce que l'écosystème Dolibarr a que nous n'avons pas

> Relevé le **9 novembre 2026** avec `scrapling`, sur [dolistore.com](https://www.dolistore.com), catégorie *Modules/Plugins*.

**Ce que Dolistore ne publie pas** : ni compteur de téléchargements, ni classement par popularité. Le seul tri offert est *date / prix / nom* (`<select id="selectProductSort">` vérifié dans le code de la page). On ne peut donc pas mesurer « les modules les plus téléchargés » — personne ne peut, à partir du site. Ce qui suit est le **catalogue entier**, c'est-à-dire tout ce qui est mesurable.

## Le chiffre

- **670 modules** au catalogue
- prix médian **85 €**, maximum **990 €**
- **17 familles** fonctionnelles

> Le classement se fait sur des mots-clés, le titre d'abord, la description ensuite. Il est **approximatif** : un module peut tomber dans la famille voisine. Le fichier `dolistore.json` porte le titre et la description de chacun, donc le classement se refait en une requête si besoin.

## Famille par famille

| Famille | Modules | Prix médian | Cordeau |
|---|---:|---:|---|
| Comptabilité / fiscalité | 16 | 79 € | **oui** |
| Banque, prélèvements, trésorerie | 6 | 90 € | **partiel** |
| RH : paie, congés, notes de frais | 1 | 299 € | **partiel** |
| Caisse, point de vente | 6 | 200 € | **oui** |
| Projets, chantiers, temps | 72 | 100 € | **oui** |
| Stock, produits, production | 61 | 90 € | **partiel** |
| Documents, GED, signature | 86 | 90 € | **partiel** |
| Portail, e-commerce, extranet | 7 | 200 € | **non** |
| Transport, livraison, logistique | 33 | 90 € | **partiel** |
| Reporting, tableaux de bord, BI | 16 | 100 € | **oui** |
| CRM, prospects, marketing | 44 | 95 € | **partiel** |
| Immobilier, hôtellerie, métiers | 19 | 220 € | **non** |
| Qualité, conformité, sécurité | 3 | 250 € | **non** |
| IA, automatisation | 49 | 75 € | **non** |
| Interface, administration, technique | 53 | 50 € | **partiel** |
| Facturation, devis, achats | 135 | 75 € | **oui** |
| Autre | 63 | 50 € | — |

## Ce que nous n'avons pas, famille par famille

Les modules sont cités du moins cher au plus cher : c'est le prix d'entrée du marché.

### Banque, prélèvements, trésorerie — 6 modules, Cordeau : partiel

- **iciSEPA** — 15 € · V20 - V24
- **MBI Monetico** — 90 € · V14 - V24
- **MBI Up2pay ETransactions** — 90 € · V14 - V24
- **PDF Annexes: Terms, bank details and datasheets added automatically to your quotes and invoices** — 149 € · V11 - V24
- **Dolibarr Advanced Treasury** — 200 € · V6 - V24
- **Banking monitoring** — — · V18 - V24

### RH : paie, congés, notes de frais — 1 modules, Cordeau : partiel

- **Recruitment Agency Pro – Complete ATS &amp; ERP for Recruitment Agencies** — 299 € · V6 - V24

### Stock, produits, production — 61 modules, Cordeau : partiel

- **Simple Stats 2.0 v13-v16** — 5 € · V13 - V16
- **InfraSTechInfos** — 20 € · V15+
- **Stock Optimizer** — 20 € · V6 - V24
- **Bondepret** — 25 € · V17 - V24
- **Clients samples** — 25 € · V19+
- **ConcatPDF (Sales terms, Product catalog, Purchase terms...)** — 25 € · V18 - V24
- **doli2doli : Data transfer between Dolibarr instances** — 25 € · V20 - V24
- **Invoice MultiWarehouse** — 25 € · V23+
- **MoreDesc : Personalised and Template Desc and Notes** — 25 € · V20 - V24
- **Import Modul Offers Customers and Products with CSV DE/EN** — 29 € · V19 - V22

### Documents, GED, signature — 86 modules, Cordeau : partiel

- **Document Tab** — 5 € · V15 - V22
- **Soc2Pdf** — 7 € · V6+
- **iciRelances** — 9 € · V23+
- **Dolishortener, URL shortener** — 10 € · V17 - V24
- **Send Document By (Web Whatsapp or Web Telegram)** — 10 € · V16 - V22
- **subscription payment receipt for Dolibarr 19** — 10 € · V19 - V20
- **PropStats - Proposal Visit Tracker** — 15 € · V22+
- **Reçu de don** — 15 € · V21 - V24
- **Bichon : Access your email archives from Dolibarr** — 25 € · V20 - V24
- **DC1 – Tender application forms from commercial proposals** — 25 € · V20 - V24

### Portail, e-commerce, extranet — 7 modules, Cordeau : non

- **ComptaPortal — Secure accountant portal** — 125 € · V18 - V24
- **CapCowork - Gestion d'espaces de coworking 1.2.8** — 150 € · V18+
- **DoliPortail** — 180 € · V23 - V24
- **Dolibarr Extranet - Website and Client Extranet** — 200 € · V6 - V24
- **Repair Shop Management – Complete Repair Workshop Platform for Dolibarr** — 299 € · V6 - V24
- **Customer portal** — 550 € · V21 - V24
- **EInvoicing (Generic FR | Official)** — — · V18 - V24

### Transport, livraison, logistique — 33 modules, Cordeau : partiel

- **Lock Shipments / Invoices** — 2 € · V4 - V16
- **Ship Smart** — 10 € · V6 - V24
- **DoliWhatsApp Link** — 15 € · V24 - V6
- **Download link to shipment and delivery** — 15 € · V21+
- **Shipment Package** — 20 € · V14 - V24
- **Professional public tickets for Dolibarr — customer submission &amp; tracking** — 40 € · V18 - V24
- **Credit limit to customers** — 50 € · V8 - V16
- **Dolistore sales follow-up** — 60 € · V19+
- **Employee Disciplinary Tracking** — 60 € · V6 - V24
- **MBI DHL Express (European Union)** — 60 € · V21 - V24

### CRM, prospects, marketing — 44 modules, Cordeau : partiel

- **Internal Proposal Status Pipeline** — 15 € · V23 - V6
- **LeadKanban — Drag&amp;drop sales pipeline** — 19 € · V14+
- **CRM Funnel** — 38 € · V20 - V22
- **Mailing Corporate** — 40 € · V14 - V19
- **DOLIcomMAP - Interactive Commercial Mapping** — 45 € · V16+
- **LemonVigie : Monitor &amp; prospect your clients, Open Data** — 49 € · V19 - V24
- **Prospecting Map** — 49 € · V18 - V24
- **Chart ERP Dolibarr** — 60 € · V6 - V24
- **Connection Hub** — 60 € · V6 - V24
- **Address Assistant AT** — 70 € · V15+

### Immobilier, hôtellerie, métiers — 19 modules, Cordeau : non

- **Societe Info** — 16 € · V6+
- **Portofolio : Advanced sales and category assignment** — 25 € · V18 - V24
- **ToDo List for customer** — 42 € · V8 - V21
- **Dropcontact Contact Enhancer** — 49 € · V16 - V24
- **Supervision** — 50 € · V4 - V15
- **click customer information** — 80 € · V19+
- **Booking – Rental &amp; Reservation management for Dolibarr** — 115 € · V18 - V24
- **Service Garage – Workshop · Repair** — 145 € · V18 - V24
- **ThirdParty Resources** — 150 € · V13 - V21
- **Factor** — 220 € · V16 - V24

### Qualité, conformité, sécurité — 3 modules, Cordeau : non

- **Quality Management for Dolibarr** — 250 € · V6 - V24
- **Quality Management for Dolibarr** — 250 € · V6 - V24
- **DoliQMS - Your Integrated Quality Management Solution for Dolibarr** — 350 € · V18 - V24

### IA, automatisation — 49 modules, Cordeau : non

- **Multiple Payment of Invoices Automatically - Advance Payment by Clients - Track Client Payment** — 8 € · V4 - V16
- **ClientGuard- Cashflow risk management** — 10 € · V17 - V24
- **Process : eyesight improvement of business workflow** — 10 € · V18+
- **Third-Party Form Elements – Enable / Disable** — 15 € · V18 - V24
- **ListGenerator : Generate query and List with AI** — 25 € · V20 - V24
- **Quantity-Based Pricing for Customer** — 29 € · V21 - V22
- **Automatic notes V3** — 30 € · V15 - V24
- **InfraSWorkflow** — 36 € · V18 - V24
- **Automatic management fees** — 38 € · V8 - V24
- **Commercial by department** — 38 € · V6 - V24

### Interface, administration, technique — 53 modules, Cordeau : partiel

- **EuroAddress - Verified address autofill (CH/FR/DE/AT/BE/LU)** — 10 € · V18+
- **Email notifications on each user login** — 15 € · V18 - V24
- **WhatsApp api** — 15 € · V23+
- **Auto Recipient Mail** — 19 € · V19+
- **Calendar event notification by email** — 20 € · V4 - V11
- **Invoices By Mail** — 20 € · V6 - V21
- **SMSPartner** — 20 € · V15 - V16
- **Third-party form auto-fill for french companies (Pappers API)** — 24 € · V11 - V24
- **Is Mail Sent** — 25 € · V13+
- **Margin View** — 30 € · V6 - V24

## Nos propres trous, relevés dans le code

| Module | Ce qui manque |
|---|---|
| `contrat` | contrats : lecture seule, ni création ni validation |
| `salaire` | bulletins de paie : lecture seule |
| `variante` | déclinaisons produit (taille, couleur) : pas d'écran |
| `recrutement` | offres et candidatures : pas d'écran |
| `reservation` | réservations : pas d'écran |
| `ressource` | ressources et affectations : pas d'écran |
| `caisse_journal` | journal de caisse : pas d'écran |
| `evenement` | validation d'un événement d'agenda : pas d'écran |
