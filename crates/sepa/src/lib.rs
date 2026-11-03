//! Virements et prélèvements SEPA : les fichiers XML que les banques lisent.
//!
//! Deux formats, ceux que Dolibarr produit et que toute banque française
//! accepte :
//!
//! - **pain.001.001.03** — virement d'un lot de paiements (ordre de transfert) ;
//! - **pain.008.001.02** — prélèvement d'un lot de créances (mandats signés).
//!
//! Aucune dépendance : le XML est assemblé à la main, échappé correctement, et
//! les montants sont en **centimes entiers** — un fichier bancaire ne supporte
//! pas d'arrondi flottant.

use std::fmt::Write as _;

/// Ce qui a empêché de produire le fichier.
#[derive(Debug, PartialEq, Eq)]
pub enum Erreur {
    /// IBAN qui ne passe pas la cle de controle (modulo 97).
    Iban(String),
    /// BIC qui n'a pas la forme attendue (8 ou 11 caracteres).
    Bic(String),
    /// Montant nul ou negatif : une banque le refuserait.
    Montant(String),
    /// Lot vide : un fichier sans operation n'a pas de sens.
    LotVide,
    /// Identifiant de créancier ICS manquant ou mal forme.
    Creancier(String),
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::Iban(i) => write!(f, "IBAN invalide : {i}"),
            Erreur::Bic(b) => write!(f, "BIC invalide : {b}"),
            Erreur::Montant(m) => write!(f, "montant invalide : {m}"),
            Erreur::LotVide => write!(f, "aucune operation dans le lot"),
            Erreur::Creancier(c) => write!(f, "identifiant de creancier invalide : {c}"),
        }
    }
}

impl std::error::Error for Erreur {}

/// Le compte qui emet : celui de l'entreprise.
#[derive(Clone, Debug)]
pub struct Emetteur {
    pub nom: String,
    pub iban: String,
    pub bic: String,
}

/// Un virement a executer.
#[derive(Clone, Debug)]
pub struct Virement {
    pub nom: String,
    pub iban: String,
    pub bic: String,
    /// Montant en centimes : 12 345 = 123,45 €.
    pub centimes: i64,
    /// Motif visible par le beneficiaire.
    pub motif: String,
    /// Reference interne (numero de facture fournisseur, par exemple).
    pub reference: String,
}

/// Un prelevement a executer, sur mandat signe.
#[derive(Clone, Debug)]
pub struct Prelevement {
    pub nom: String,
    pub iban: String,
    pub bic: Option<String>,
    pub centimes: i64,
    pub motif: String,
    /// Reference unique du mandat (celle du contrat signe).
    pub mandat: String,
    /// Date de signature du mandat, `AAAA-MM-JJ`.
    pub mandat_signe_le: String,
    /// `true` pour un prelevement recurrent, `false` pour un ponctuel.
    pub recurrent: bool,
}

/// Verifie un IBAN : longueur, alphanumerique, cle modulo 97.
///
/// La regle est simple et sans exception : on deplace les quatre premiers
/// caracteres a la fin, on remplace chaque lettre par sa valeur (A=10 … Z=35),
/// et le nombre obtenu doit etre congru a 1 modulo 97.
pub fn iban_valide(iban: &str) -> bool {
    let propre: String = iban
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    if propre.len() < 15 || propre.len() > 34 {
        return false;
    }
    if !propre.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let (tete, queue) = propre.split_at(4);
    let rearranged = format!("{queue}{tete}");

    // Le nombre depasse largement u128 : on reduit modulo 97 chiffre par chiffre.
    let mut reste: u32 = 0;
    for c in rearranged.chars() {
        let valeur = match c {
            '0'..='9' => (c as u8 - b'0') as u32,
            'A'..='Z' => (c as u8 - b'A') as u32 + 10,
            _ => return false,
        };
        if valeur >= 10 {
            reste = (reste * 100 + valeur) % 97;
        } else {
            reste = (reste * 10 + valeur) % 97;
        }
    }
    reste == 1
}

/// Verifie un BIC : 8 ou 11 caracteres, lettres et chiffres.
pub fn bic_valide(bic: &str) -> bool {
    let b = bic.trim();
    matches!(b.len(), 8 | 11) && b.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Verifie un identifiant de créancier SEPA : `FRxxZZZ` suivi du numero.
pub fn creancier_valide(ics: &str) -> bool {
    let i = ics.trim().to_uppercase();
    i.len() >= 8
        && i.len() <= 35
        && i.chars().all(|c| c.is_ascii_alphanumeric())
        && i.starts_with(char::is_alphabetic)
}

/// Echappe le texte pour du XML : sans ça, un `&` dans un nom de client casse
/// le fichier et la banque rejette tout le lot.
fn echappe(s: &str) -> String {
    let mut sortie = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => sortie.push_str("&amp;"),
            '<' => sortie.push_str("&lt;"),
            '>' => sortie.push_str("&gt;"),
            '"' => sortie.push_str("&quot;"),
            '\'' => sortie.push_str("&apos;"),
            autre => sortie.push(autre),
        }
    }
    sortie
}

/// Montant SEPA : `1234.56`, deux decimales, point decimal.
fn montant(centimes: i64) -> String {
    format!("{}.{:02}", centimes / 100, (centimes % 100).abs())
}

fn date_iso(date: &str) -> Result<String, Erreur> {
    // On attend AAAA-MM-JJ ; on refuse le reste plutot que de produire un
    // fichier que la banque rejettera plus tard, sans explication.
    let d = date.trim();
    let ok = d.len() == 10
        && d.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        });
    if ok {
        Ok(d.to_string())
    } else {
        Err(Erreur::Montant(format!("date attendue AAAA-MM-JJ : {date}")))
    }
}

/// Assemble un fichier de virement (pain.001.001.03).
pub fn virement_xml(
    emetteur: &Emetteur,
    execution: &str,
    lot: &[Virement],
) -> Result<String, Erreur> {
    if lot.is_empty() {
        return Err(Erreur::LotVide);
    }
    if !iban_valide(&emetteur.iban) {
        return Err(Erreur::Iban(emetteur.iban.clone()));
    }
    if !bic_valide(&emetteur.bic) {
        return Err(Erreur::Bic(emetteur.bic.clone()));
    }
    let date = date_iso(execution)?;
    let total: i64 = lot.iter().map(|v| v.centimes).sum();

    let mut x = String::new();
    let _ = write!(
        x,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.001.001.03\">\n",
            "  <CstmrCdtTrfInitn>\n",
            "    <GrpHdr>\n",
            "      <MsgId>VIR-{date}-{n}</MsgId>\n",
            "      <CreDtTm>{date}T00:00:00</CreDtTm>\n",
            "      <NbOfTxs>{n}</NbOfTxs>\n",
            "      <CtrlSum>{total}</CtrlSum>\n",
            "      <InitgPty><Nm>{emetteur}</Nm></InitgPty>\n",
            "    </GrpHdr>\n",
            "    <PmtInf>\n",
            "      <PmtInfId>VIR-{date}-{n}</PmtInfId>\n",
            "      <PmtMtd>TRF</PmtMtd>\n",
            "      <NbOfTxs>{n}</NbOfTxs>\n",
            "      <CtrlSum>{total}</CtrlSum>\n",
            "      <ReqdExctnDt>{date}</ReqdExctnDt>\n",
            "      <Dbtr><Nm>{emetteur}</Nm></Dbtr>\n",
            "      <DbtrAcct><Id><IBAN>{iban}</IBAN></Id></DbtrAcct>\n",
            "      <DbtrAgt><FinInstnId><BIC>{bic}</BIC></FinInstnId></DbtrAgt>\n",
        ),
        date = date,
        n = lot.len(),
        total = montant(total),
        emetteur = echappe(&emetteur.nom),
        iban = echappe(&emetteur.iban.replace(' ', "")),
        bic = echappe(&emetteur.bic),
    );

    for v in lot {
        if v.centimes <= 0 {
            return Err(Erreur::Montant(v.reference.clone()));
        }
        if !iban_valide(&v.iban) {
            return Err(Erreur::Iban(v.iban.clone()));
        }
        if !bic_valide(&v.bic) {
            return Err(Erreur::Bic(v.bic.clone()));
        }
        let _ = write!(
            x,
            concat!(
                "      <CdtTrfTxInf>\n",
                "        <PmtId><EndToEndId>{reference}</EndToEndId></PmtId>\n",
                "        <Amt><InstdAmt Ccy=\"EUR\">{montant}</InstdAmt></Amt>\n",
                "        <CdtrAgt><FinInstnId><BIC>{bic}</BIC></FinInstnId></CdtrAgt>\n",
                "        <Cdtr><Nm>{nom}</Nm></Cdtr>\n",
                "        <CdtrAcct><Id><IBAN>{iban}</IBAN></Id></CdtrAcct>\n",
                "        <RmtInf><Ustrd>{motif}</Ustrd></RmtInf>\n",
                "      </CdtTrfTxInf>\n",
            ),
            reference = echappe(&v.reference),
            montant = montant(v.centimes),
            bic = echappe(&v.bic),
            nom = echappe(&v.nom),
            iban = echappe(&v.iban.replace(' ', "")),
            motif = echappe(&v.motif),
        );
    }

    x.push_str("    </PmtInf>\n  </CstmrCdtTrfInitn>\n</Document>\n");
    Ok(x)
}

/// Assemble un fichier de prelevement (pain.008.001.02).
pub fn prelevement_xml(
    emetteur: &Emetteur,
    id_creancier: &str,
    execution: &str,
    lot: &[Prelevement],
) -> Result<String, Erreur> {
    if lot.is_empty() {
        return Err(Erreur::LotVide);
    }
    if !creancier_valide(id_creancier) {
        return Err(Erreur::Creancier(id_creancier.to_string()));
    }
    if !iban_valide(&emetteur.iban) {
        return Err(Erreur::Iban(emetteur.iban.clone()));
    }
    let date = date_iso(execution)?;
    let total: i64 = lot.iter().map(|p| p.centimes).sum();
    let recurrent = lot.iter().any(|p| p.recurrent);
    let sequence = if recurrent { "RCUR" } else { "FRST" };

    let mut x = String::new();
    let _ = write!(
        x,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.008.001.02\">\n",
            "  <CstmrDrctDbtInitn>\n",
            "    <GrpHdr>\n",
            "      <MsgId>PRL-{date}-{n}</MsgId>\n",
            "      <CreDtTm>{date}T00:00:00</CreDtTm>\n",
            "      <NbOfTxs>{n}</NbOfTxs>\n",
            "      <CtrlSum>{total}</CtrlSum>\n",
            "      <InitgPty><Nm>{emetteur}</Nm></InitgPty>\n",
            "    </GrpHdr>\n",
            "    <PmtInf>\n",
            "      <PmtInfId>PRL-{date}-{n}</PmtInfId>\n",
            "      <PmtMtd>DD</PmtMtd>\n",
            "      <NbOfTxs>{n}</NbOfTxs>\n",
            "      <CtrlSum>{total}</CtrlSum>\n",
            "      <PmtTpInf><SvcLvl><Cd>SEPA</Cd></SvcLvl>",
            "<SeqTp>{sequence}</SeqTp></PmtTpInf>\n",
            "      <ReqdColltnDt>{date}</ReqdColltnDt>\n",
            "      <Cdtr><Nm>{emetteur}</Nm></Cdtr>\n",
            "      <CdtrAcct><Id><IBAN>{iban}</IBAN></Id></CdtrAcct>\n",
            "      <CdtrAgt><FinInstnId><BIC>{bic}</BIC></FinInstnId></CdtrAgt>\n",
            "      <ChrgtBr>SEPA</ChrgtBr>\n",
            "      <CdtrSchmeId><Id><PrvtId><Othr>",
            "<Id>{ics}</Id><SchmeNm><Prtry>SEPA</Prtry></SchmeNm>",
            "</Othr></PrvtId></Id></CdtrSchmeId>\n",
        ),
        date = date,
        n = lot.len(),
        total = montant(total),
        sequence = sequence,
        emetteur = echappe(&emetteur.nom),
        iban = echappe(&emetteur.iban.replace(' ', "")),
        bic = echappe(&emetteur.bic),
        ics = echappe(id_creancier),
    );

    for p in lot {
        if p.centimes <= 0 {
            return Err(Erreur::Montant(p.mandat.clone()));
        }
        if !iban_valide(&p.iban) {
            return Err(Erreur::Iban(p.iban.clone()));
        }
        let _ = write!(
            x,
            concat!(
                "      <DrctDbtTxInf>\n",
                "        <PmtId><EndToEndId>{mandat}</EndToEndId></PmtId>\n",
                "        <InstdAmt Ccy=\"EUR\">{montant}</InstdAmt>\n",
                "        <DrctDbtTx><MndtRltdInf>",
                "<MndtId>{mandat}</MndtId>",
                "<DtOfSgntr>{signe}</DtOfSgntr>",
                "<AmdmntInd>false</AmdmntInd>",
                "</MndtRltdInf></DrctDbtTx>\n",
                "        <DbtrAgt><FinInstnId><BIC>{bic}</BIC></FinInstnId></DbtrAgt>\n",
                "        <Dbtr><Nm>{nom}</Nm></Dbtr>\n",
                "        <DbtrAcct><Id><IBAN>{iban}</IBAN></Id></DbtrAcct>\n",
                "        <RmtInf><Ustrd>{motif}</Ustrd></RmtInf>\n",
                "      </DrctDbtTxInf>\n",
            ),
            mandat = echappe(&p.mandat),
            montant = montant(p.centimes),
            signe = echappe(&p.mandat_signe_le),
            bic = echappe(p.bic.as_deref().unwrap_or_default()),
            nom = echappe(&p.nom),
            iban = echappe(&p.iban.replace(' ', "")),
            motif = echappe(&p.motif),
        );
    }

    x.push_str("    </PmtInf>\n  </CstmrDrctDbtInitn>\n</Document>\n");
    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emetteur() -> Emetteur {
        Emetteur {
            nom: "Pierre Angulaire".into(),
            iban: "FR7616807003483848689521643".into(),
            bic: "CCBPFRPPBDX".into(),
        }
    }

    #[test]
    fn les_ibans_reels_passent_et_les_faux_tombent() {
        // IBAN reels, dont celui du compte de l'entreprise.
        assert!(iban_valide("FR7616807003483848689521643"));
        assert!(iban_valide("FR76 1680 7003 4838 4868 9521 643"));
        assert!(iban_valide("DE89370400440532013000"));
        assert!(iban_valide("GB82WEST12345698765432"));
        // Un chiffre change : la cle ne tombe plus juste.
        assert!(!iban_valide("FR7616807003483848689521644"));
        // Trop court, caracteres bizarres.
        assert!(!iban_valide("FR76"));
        assert!(!iban_valide("FR76168070034838486895216!3"));
    }

    #[test]
    fn un_bic_a_huit_ou_onze_caracteres() {
        assert!(bic_valide("CCBPFRPPBDX"));
        assert!(bic_valide("CCBPFRPP"));
        assert!(!bic_valide("CCBPFRPPBD"));
        assert!(!bic_valide("CCBP-FRPP"));
    }

    #[test]
    fn le_virement_porte_les_montants_et_les_beneficiaires() {
        let lot = vec![
            Virement {
                nom: "Point P & Cie".into(),
                iban: "FR1420041010050500013M02606".into(),
                bic: "CCBPFRPPBDX".into(),
                centimes: 123_456,
                motif: "Facture F-2611-0012".into(),
                reference: "F-2611-0012".into(),
            },
            Virement {
                nom: "Samse".into(),
                iban: "DE89370400440532013000".into(),
                bic: "CCBPFRPPBDX".into(),
                centimes: 7_500,
                motif: "Facture F-2611-0013".into(),
                reference: "F-2611-0013".into(),
            },
        ];
        let xml = virement_xml(&emetteur(), "2026-11-10", &lot).unwrap();
        assert!(xml.contains("pain.001.001.03"));
        assert!(xml.contains("<NbOfTxs>2</NbOfTxs>"));
        // 1234,56 + 75,00 = 1309,56
        assert!(xml.contains("<CtrlSum>1309.56</CtrlSum>"));
        assert!(xml.contains("<InstdAmt Ccy=\"EUR\">1234.56</InstdAmt>"));
        assert!(xml.contains("Point P &amp; Cie"), "le & doit etre echappe");
        assert!(xml.contains("<BIC>CCBPFRPPBDX</BIC>"));
        assert!(xml.contains("2026-11-10"));
    }

    #[test]
    fn un_lot_vide_ou_un_montant_nul_sont_refuses() {
        assert_eq!(virement_xml(&emetteur(), "2026-11-10", &[]), Err(Erreur::LotVide));
        let lot = vec![Virement {
            nom: "X".into(),
            iban: "FR1420041010050500013M02606".into(),
            bic: "CCBPFRPPBDX".into(),
            centimes: 0,
            motif: "rien".into(),
            reference: "R1".into(),
        }];
        assert!(matches!(
            virement_xml(&emetteur(), "2026-11-10", &lot),
            Err(Erreur::Montant(_))
        ));
    }

    #[test]
    fn une_date_mal_ecrite_est_refusee_avant_la_banque() {
        let lot = vec![Virement {
            nom: "X".into(),
            iban: "FR1420041010050500013M02606".into(),
            bic: "CCBPFRPPBDX".into(),
            centimes: 100,
            motif: "m".into(),
            reference: "R1".into(),
        }];
        assert!(virement_xml(&emetteur(), "10/11/2026", &lot).is_err());
    }

    #[test]
    fn le_prelevement_porte_le_mandat_et_le_creancier() {
        let lot = vec![Prelevement {
            nom: "M. Dupont".into(),
            iban: "FR1420041010050500013M02606".into(),
            bic: Some("CCBPFRPPBDX".into()),
            centimes: 50_000,
            motif: "Echéance contrat 2026-11".into(),
            mandat: "MAN-2026-0007".into(),
            mandat_signe_le: "2026-01-15".into(),
            recurrent: true,
        }];
        let xml = prelevement_xml(&emetteur(), "FR12ZZZ123456", "2026-11-10", &lot).unwrap();
        assert!(xml.contains("pain.008.001.02"));
        assert!(xml.contains("<SeqTp>RCUR</SeqTp>"));
        assert!(xml.contains("<MndtId>MAN-2026-0007</MndtId>"));
        assert!(xml.contains("<CtrlSum>500.00</CtrlSum>"));
    }

    #[test]
    fn un_identifiant_de_creancier_fautif_est_refuse() {
        let lot = vec![Prelevement {
            nom: "X".into(),
            iban: "FR1420041010050500013M02606".into(),
            bic: None,
            centimes: 100,
            motif: "m".into(),
            mandat: "M1".into(),
            mandat_signe_le: "2026-01-01".into(),
            recurrent: false,
        }];
        assert!(matches!(
            prelevement_xml(&emetteur(), "12", "2026-11-10", &lot),
            Err(Erreur::Creancier(_))
        ));
    }
}
