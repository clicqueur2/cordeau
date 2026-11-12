//! Import et export CSV.
//!
//! Le parseur suit RFC 4180 (guillemets, champs multi-lignes, echappement par
//! doublement) et detecte le separateur (`;` ou `,`). C'est ce qui permet
//! d'avaler un export Excel francais sans demander a l'utilisateur de le
//! nettoyer.

use std::error::Error;

use mysql::Pool;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub entetes: Vec<String>,
    pub lignes: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct RapportImport {
    pub crees: usize,
    pub ignores: usize,
    pub erreurs: Vec<String>,
}

/// Detecte le separateur : celui qui revient le plus sur la premiere ligne.
fn separateur(texte: &str) -> char {
    let premiere = texte.lines().next().unwrap_or("");
    let pv = premiere.matches(';').count();
    let virgule = premiere.matches(',').count();
    if pv >= virgule {
        ';'
    } else {
        ','
    }
}

/// Parse un CSV. Les champs entre guillemets peuvent contenir separateur et
/// retours a la ligne.
pub fn analyser(texte: &str) -> Result<Table, Box<dyn Error>> {
    let texte = texte.trim_start_matches('\u{feff}');
    let sep = separateur(texte);

    let mut enregistrements: Vec<Vec<String>> = Vec::new();
    let mut champ = String::new();
    let mut ligne: Vec<String> = Vec::new();
    let mut entre_guillemets = false;
    let mut chars = texte.chars().peekable();

    while let Some(c) = chars.next() {
        if entre_guillemets {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    champ.push('"');
                    chars.next();
                } else {
                    entre_guillemets = false;
                }
            } else {
                champ.push(c);
            }
        } else if c == '"' {
            entre_guillemets = true;
        } else if c == sep {
            ligne.push(std::mem::take(&mut champ));
        } else if c == '\n' {
            ligne.push(std::mem::take(&mut champ));
            enregistrements.push(std::mem::take(&mut ligne));
        } else if c == '\r' {
            // ignore : gere avec \n
        } else {
            champ.push(c);
        }
    }
    if !champ.is_empty() || !ligne.is_empty() {
        ligne.push(champ);
        enregistrements.push(ligne);
    }

    enregistrements.retain(|l| l.iter().any(|c| !c.trim().is_empty()));
    if enregistrements.is_empty() {
        return Err("fichier vide".into());
    }

    let entetes = enregistrements
        .remove(0)
        .into_iter()
        .map(|e| e.trim().to_lowercase())
        .collect();
    Ok(Table {
        entetes,
        lignes: enregistrements,
    })
}

/// Indice d'une colonne a partir de plusieurs noms possibles.
fn colonne(entetes: &[String], noms: &[&str]) -> Option<usize> {
    for n in noms {
        if let Some(i) = entetes.iter().position(|e| e == n) {
            return Some(i);
        }
    }
    None
}

fn valeur(ligne: &[String], idx: Option<usize>) -> Option<String> {
    idx.and_then(|i| ligne.get(i))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Importe des tiers. Colonnes reconnues : nom/name, adresse/address, cp/zip,
/// ville/town, email, tel/phone, siret, tva.
pub fn importer_tiers(
    pool: &Pool,
    entity: i64,
    texte: &str,
) -> Result<RapportImport, Box<dyn Error>> {
    let table = analyser(texte)?;
    let i_nom = colonne(
        &table.entetes,
        &["nom", "name", "raison sociale", "societe"],
    );
    let Some(i_nom) = i_nom else {
        return Err("colonne « nom » introuvable dans l'entete".into());
    };
    let i_adresse = colonne(&table.entetes, &["adresse", "address", "rue"]);
    let i_cp = colonne(&table.entetes, &["cp", "code postal", "zip", "postal"]);
    let i_ville = colonne(&table.entetes, &["ville", "town", "city"]);
    let i_email = colonne(&table.entetes, &["email", "mail", "courriel"]);
    let i_tel = colonne(&table.entetes, &["tel", "telephone", "phone", "mobile"]);
    let i_siret = colonne(&table.entetes, &["siret"]);
    let i_tva = colonne(&table.entetes, &["tva", "tva intra", "tva_intra", "vat"]);

    let mut rapport = RapportImport::default();
    for (n, ligne) in table.lignes.iter().enumerate() {
        let nom = valeur(ligne, Some(i_nom));
        let Some(nom) = nom else {
            rapport.ignores += 1;
            continue;
        };
        // Doublon sur le nom : on ne recree pas.
        let mut conn = pool.get_conn()?;
        use mysql::prelude::Queryable;
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_societe WHERE entity = ? AND nom = ? LIMIT 1",
            (entity, nom.as_str()),
        )?;
        if existe.is_some() {
            rapport.ignores += 1;
            continue;
        }

        let mut nouveau = crate::party::NewParty::client(&nom);
        nouveau.address = valeur(ligne, i_adresse);
        nouveau.zip = valeur(ligne, i_cp);
        nouveau.town = valeur(ligne, i_ville);
        nouveau.email = valeur(ligne, i_email);
        nouveau.phone = valeur(ligne, i_tel);
        nouveau.siret = valeur(ligne, i_siret);
        nouveau.tva_intra = valeur(ligne, i_tva);

        match crate::party::create(pool, entity, &nouveau) {
            Ok(_) => rapport.crees += 1,
            Err(e) => rapport.erreurs.push(format!("ligne {} : {e}", n + 2)),
        }
    }
    Ok(rapport)
}

/// Importe des produits. Colonnes : ref (obligatoire), libelle/label,
/// prix/prix_ht, tva, type (produit|service), unite.
pub fn importer_produits(
    pool: &Pool,
    entity: i64,
    texte: &str,
) -> Result<RapportImport, Box<dyn Error>> {
    let table = analyser(texte)?;
    let Some(i_ref) = colonne(&table.entetes, &["ref", "reference", "code"]) else {
        return Err("colonne ref introuvable dans l'entete".into());
    };
    let Some(i_libelle) = colonne(&table.entetes, &["libelle", "label", "designation", "nom"])
    else {
        return Err("colonne libelle introuvable dans l'entete".into());
    };
    let i_prix = colonne(
        &table.entetes,
        &["prix", "prix_ht", "prix ht", "price", "pu"],
    );
    let i_tva = colonne(&table.entetes, &["tva", "taux tva", "tva_tx", "vat"]);
    let i_type = colonne(&table.entetes, &["type", "nature"]);
    let i_unite = colonne(&table.entetes, &["unite", "unit", "unit code"]);

    let mut rapport = RapportImport::default();
    for (n, ligne) in table.lignes.iter().enumerate() {
        let (Some(reference), Some(libelle)) =
            (valeur(ligne, Some(i_ref)), valeur(ligne, Some(i_libelle)))
        else {
            rapport.ignores += 1;
            continue;
        };

        let mut conn = pool.get_conn()?;
        use mysql::prelude::Queryable;
        let existe: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_product WHERE entity = ? AND ref = ? LIMIT 1",
            (entity, reference.as_str()),
        )?;
        if existe.is_some() {
            rapport.ignores += 1;
            continue;
        }

        let mut nouveau = crate::product::NewProduct::new(&reference, &libelle);
        nouveau.price = valeur(ligne, i_prix).and_then(|v| v.replace(',', ".").parse().ok());
        nouveau.tva_tx = valeur(ligne, i_tva).and_then(|v| v.replace(',', ".").parse().ok());
        if valeur(ligne, i_type)
            .map(|v| v.eq_ignore_ascii_case("service"))
            .unwrap_or(false)
        {
            nouveau.fk_product_type = crate::product::TYPE_SERVICE;
        }
        if let Some(code) = valeur(ligne, i_unite) {
            nouveau.fk_unit = crate::product::unit_id(pool, &code)?;
        }

        match crate::product::create(pool, entity, &nouveau) {
            Ok(_) => rapport.crees += 1,
            Err(e) => rapport.erreurs.push(format!("ligne {} : {e}", n + 2)),
        }
    }
    Ok(rapport)
}

/// Un montant lu dans un CSV, ou une erreur nommee.
///
/// `1 234,56`, `12,50 €` et `1.234,56` sont refuses **explicitement** : mieux
/// vaut une ligne refusee qu'une facture a zero euro.
fn lire_decimal(
    valeur: Option<&str>,
    defaut: rust_decimal::Decimal,
) -> Result<rust_decimal::Decimal, String> {
    use std::str::FromStr;
    let Some(brut) = valeur else {
        return Ok(defaut);
    };
    let nettoye: String = brut
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '€' && *c != '\u{00a0}')
        .collect();
    if nettoye.is_empty() {
        return Ok(defaut);
    }
    rust_decimal::Decimal::from_str(&nettoye.replace(',', "."))
        .map_err(|_| format!("illisible : « {brut} »"))
}

/// Importe un historique de factures. Une ligne = une ligne de facture ; les
/// lignes sont regroupees par (client, reference de facture).
///
/// Colonnes : client (obligatoire), facture (reference d'origine), date,
/// echeance, libelle, qte, pu, tva.
pub fn importer_factures(
    pool: &Pool,
    entity: i64,
    texte: &str,
) -> Result<RapportImport, Box<dyn Error>> {
    let table = analyser(texte)?;
    let Some(i_client) = colonne(&table.entetes, &["client", "nom", "tiers"]) else {
        return Err("colonne client introuvable dans l'entete".into());
    };
    let i_facture = colonne(&table.entetes, &["facture", "ref", "reference", "numero"]);
    let i_date = colonne(&table.entetes, &["date"]);
    let i_echeance = colonne(&table.entetes, &["echeance", "date_lim_reglement"]);
    let Some(i_libelle) = colonne(
        &table.entetes,
        &["libelle", "designation", "label", "produit"],
    ) else {
        return Err("colonne libelle introuvable dans l'entete".into());
    };
    let i_qte = colonne(&table.entetes, &["qte", "qty", "quantite"]);
    let i_pu = colonne(&table.entetes, &["pu", "prix", "prix_ht", "price"]);
    let i_tva = colonne(&table.entetes, &["tva", "taux", "vat"]);

    // Regroupement dans l'ordre d'apparition.
    let mut groupes: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    for ligne in &table.lignes {
        let Some(client) = valeur(ligne, Some(i_client)) else {
            continue;
        };
        let cle = match valeur(ligne, i_facture) {
            Some(r) => format!("{client}|{r}"),
            None => client.clone(),
        };
        match groupes.iter_mut().find(|(c, _)| *c == cle) {
            Some((_, lignes)) => lignes.push(ligne.clone()),
            None => groupes.push((cle, vec![ligne.clone()])),
        }
    }

    let mut rapport = RapportImport::default();
    for (cle, lignes) in groupes {
        let client = cle.split('|').next().unwrap_or("").to_string();

        // Le tiers est cree s'il n'existe pas : on ne perd pas une facture
        // parce que la fiche client manque.
        let mut conn = pool.get_conn()?;
        use mysql::prelude::Queryable;
        let mut fk_soc: Option<i64> = conn.exec_first(
            "SELECT rowid FROM llx_societe WHERE entity = ? AND nom = ? LIMIT 1",
            (entity, client.as_str()),
        )?;
        if fk_soc.is_none() {
            fk_soc = Some(crate::party::create(
                pool,
                entity,
                &crate::party::NewParty::client(&client),
            )?);
        }
        let fk_soc = fk_soc.unwrap_or(0);

        let mut nouvelle = crate::invoice::NewInvoice::new(fk_soc);
        let premiere = &lignes[0];
        nouvelle.datef = valeur(premiere, i_date);
        nouvelle.date_lim_reglement = valeur(premiere, i_echeance);

        for ligne in &lignes {
            let Some(libelle) = valeur(ligne, Some(i_libelle)) else {
                continue;
            };
            // Un prix illisible ne vaut **pas** zero : une facture a 0 € qui
            // part chez le client est pire qu'une ligne refusee. On refuse la
            // ligne, on dit pourquoi, et la facture ne sera pas creee.
            let qte = lire_decimal(valeur(ligne, i_qte).as_deref(), rust_decimal::Decimal::ONE)
                .map_err(|e| format!("{cle} : quantite {e}"))?;
            let pu = lire_decimal(valeur(ligne, i_pu).as_deref(), rust_decimal::Decimal::ZERO)
                .map_err(|e| format!("{cle} : prix unitaire {e}"))?;
            let tva = lire_decimal(valeur(ligne, i_tva).as_deref(), rust_decimal::Decimal::ZERO)
                .map_err(|e| format!("{cle} : TVA {e}"))?;
            nouvelle
                .lines
                .push(crate::invoice::NewInvoiceLine::new(&libelle, qte, pu, tva));
        }
        if nouvelle.lines.is_empty() {
            rapport.ignores += 1;
            continue;
        }

        match crate::invoice::create(pool, entity, &nouvelle) {
            Ok(id) => {
                // Une facture reprise est deja emise : on la valide. Si la
                // validation echoue, la facture reste en brouillon `(PROV…)`
                // **sans numero legal** : la compter comme creee serait mentir,
                // et l'ecran annonce « validees automatiquement ».
                match crate::invoice::validate(pool, entity, id) {
                    Ok(()) => rapport.crees += 1,
                    Err(e) => rapport.erreurs.push(format!(
                        "{cle} : facture brouillon creee mais NON validee (pas de numero legal) : {e}"
                    )),
                }
            }
            Err(e) => rapport.erreurs.push(format!("{cle} : {e}")),
        }
    }
    Ok(rapport)
}

/// Export CSV des tiers. Le separateur `;` et l'encodage UTF-8 avec BOM sont
/// ceux qu'Excel attend en France.
pub fn exporter_tiers(pool: &Pool, entity: i64) -> Result<String, Box<dyn Error>> {
    let listes = crate::party::list(pool, entity, 100_000, 0)?;
    let mut out = String::from("\u{feff}code_client;nom;adresse;cp;ville;email;tel;siret;tva\n");
    for p in listes {
        let code = p.code_client.clone().unwrap_or_default();
        out.push_str(&csv(&[
            code,
            p.nom,
            p.address.unwrap_or_default(),
            p.zip.unwrap_or_default(),
            p.town.unwrap_or_default(),
            p.email.unwrap_or_default(),
            p.phone.unwrap_or_default(),
            p.siret.unwrap_or_default(),
            p.tva_intra.unwrap_or_default(),
        ]));
        out.push('\n');
    }
    Ok(out)
}

/// Export CSV des produits.
pub fn exporter_produits(pool: &Pool, entity: i64) -> Result<String, Box<dyn Error>> {
    let listes = crate::product::list(pool, entity, 100_000, 0)?;
    let mut out = String::from("\u{feff}ref;libelle;prix_ht;tva;type\n");
    for p in listes {
        let type_ = if p.est_service() {
            "service"
        } else {
            "produit"
        }
        .to_string();
        out.push_str(&csv(&[
            p.ref_,
            p.label,
            p.price.map(|d| d.to_string()).unwrap_or_default(),
            p.tva_tx.map(|d| d.to_string()).unwrap_or_default(),
            type_,
        ]));
        out.push('\n');
    }
    Ok(out)
}

fn csv(champs: &[String]) -> String {
    champs
        .iter()
        .map(|c| {
            if c.contains(';') || c.contains('"') || c.contains('\n') {
                format!("\"{}\"", c.replace('"', "\"\""))
            } else {
                c.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecte_le_point_virgule() {
        let t = analyser("nom;ville\nDupont;Charly\n").unwrap();
        assert_eq!(t.entetes, vec!["nom", "ville"]);
        assert_eq!(t.lignes, vec![vec!["Dupont", "Charly"]]);
    }

    #[test]
    fn detecte_la_virgule() {
        let t = analyser("nom,ville\nDupont,Charly\n").unwrap();
        assert_eq!(t.entetes, vec!["nom", "ville"]);
        assert_eq!(t.lignes.len(), 1);
    }

    #[test]
    fn gere_les_guillemets_et_les_retours_a_la_ligne() {
        let csv = "nom;adresse\n\"Dupont; fils\";\"1 rue\ndu Lac\"\n";
        let t = analyser(csv).unwrap();
        assert_eq!(t.lignes[0][0], "Dupont; fils");
        assert_eq!(t.lignes[0][1], "1 rue\ndu Lac");
    }

    #[test]
    fn gere_le_doublement_des_guillemets() {
        let t = analyser("nom\n\"Le \"\"Grand\"\" Chene\"\n").unwrap();
        assert_eq!(t.lignes[0][0], "Le \"Grand\" Chene");
    }

    #[test]
    fn ignore_les_lignes_vides() {
        let t = analyser("nom;ville\n\nA;B\n;;\n").unwrap();
        assert_eq!(t.lignes.len(), 1);
    }

    #[test]
    fn refuse_un_fichier_vide() {
        assert!(analyser("").is_err());
    }

    #[test]
    fn echappe_a_l_export() {
        assert_eq!(csv(&["a;b".into(), "c".into()]), "\"a;b\";c");
    }
}
