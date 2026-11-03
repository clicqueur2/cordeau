//! Ticket de caisse en texte, pour une imprimante thermique 80 mm.
//!
//! Une imprimante thermique n'imprime pas de PDF : elle imprime du texte en
//! 42 ou 48 colonnes. Ce module produit exactement ce que la machine attend,
//! et ce que la loi exige sur un ticket :
//!
//! - le **recapitulatif par taux de TVA** (obligatoire en France) ;
//! - le total HT, le total TVA, le total TTC ;
//! - les mentions de la societe : nom, adresse, SIRET, TVA intracommunautaire ;
//! - le numero de ticket et la date.
//!
//! Tout est en largeur fixe : un libelle trop long est coupe, pas deborde.

use std::fmt::Write as _;

use rust_decimal::Decimal;

use crate::Panier;

/// Largeur utile d'une imprimante 80 mm : 42 colonnes en police normale.
pub const LARGEUR: usize = 42;

/// Ce qui s'imprime en tete et en pied.
#[derive(Clone, Debug)]
pub struct Entete {
    pub societe: String,
    pub adresse: String,
    pub siret: String,
    pub tva_intra: String,
    /// Numero du ticket, dans la serie de la caisse.
    pub numero: String,
    /// Date et heure, deja formatees.
    pub date: String,
    /// Nom de la caisse ou du vendeur.
    pub caisse: String,
}

/// Coupe ou complete un texte a la largeur demandee.
fn largeur_texte(s: &str, n: usize) -> String {
    let propre: String = s.chars().take(n).collect();
    let manque = n.saturating_sub(propre.chars().count());
    format!("{propre}{}", " ".repeat(manque))
}

/// Aligne a gauche un libelle et a droite un montant, sur la largeur du ticket.
fn ligne(libelle: &str, montant: &str) -> String {
    let place = LARGEUR.saturating_sub(montant.chars().count() + 1);
    format!("{}{} {montant}\n", largeur_texte(libelle, place), "")
}

/// Formate un montant a la francaise : `1 234,56`.
fn euro(montant: Decimal) -> String {
    let arrondi = montant.round_dp(2);
    let negatif = arrondi.is_sign_negative();
    let absolu = arrondi.abs();
    let entier = absolu.trunc();
    let centimes = (absolu - entier) * Decimal::from(100);
    let centimes = centimes.round().to_string();
    let entier = entier.to_string();

    // Espace insecable fine entre les milliers, comme sur un ticket imprime.
    let mut groupe = String::new();
    for (i, c) in entier.chars().enumerate() {
        if i > 0 && (entier.len() - i).is_multiple_of(3) {
            groupe.push(' ');
        }
        groupe.push(c);
    }
    let signe = if negatif { "-" } else { "" };
    format!("{signe}{groupe},{:0>2}", centimes)
}

/// Produit le ticket complet.
pub fn ticket_texte(entete: &Entete, panier: &Panier, moyen: &str, paye: Decimal) -> String {
    let mut t = String::new();

    // --- en-tete ---
    for l in [&entete.societe, &entete.adresse] {
        if !l.is_empty() {
            let _ = writeln!(t, "{}", centre(l));
        }
    }
    if !entete.siret.is_empty() {
        let _ = writeln!(t, "{}", centre(&format!("SIRET {}", entete.siret)));
    }
    if !entete.tva_intra.is_empty() {
        let _ = writeln!(t, "{}", centre(&format!("TVA {}", entete.tva_intra)));
    }
    let _ = writeln!(t, "{}", "-".repeat(LARGEUR));

    let _ = writeln!(t, "Ticket {}   {}", entete.numero, entete.date);
    if !entete.caisse.is_empty() {
        let _ = writeln!(t, "Caisse {}", entete.caisse);
    }
    let _ = writeln!(t, "{}", "-".repeat(LARGEUR));

    // --- lignes ---
    for (l, d) in panier.lignes().iter().zip(panier.detail()) {
        let _ = writeln!(t, "{}", largeur_texte(&l.libelle, LARGEUR));
        let quantite = format!(
            "  {} x {}",
            l.quantite.normalize(),
            euro(l.prix_unitaire_ht)
        );
        t.push_str(&ligne(&quantite, &euro(d.ttc)));
    }

    let _ = writeln!(t, "{}", "-".repeat(LARGEUR));

    // --- recapitulatif par taux de TVA (obligatoire) ---
    let mut par_taux: std::collections::BTreeMap<String, (Decimal, Decimal)> =
        std::collections::BTreeMap::new();
    for (l, d) in panier.lignes().iter().zip(panier.detail()) {
        let cle = l.taux_tva.normalize().to_string();
        let entree = par_taux
            .entry(cle)
            .or_insert((Decimal::ZERO, Decimal::ZERO));
        entree.0 += d.ht;
        entree.1 += d.tva;
    }
    let _ = writeln!(t, "TVA   base HT      taux      montant");
    for (taux, (base, montant)) in &par_taux {
        let _ = writeln!(
            t,
            "      {:>10}  {:>5} %  {:>10}",
            euro(*base),
            taux,
            euro(*montant)
        );
    }

    let _ = writeln!(t, "{}", "-".repeat(LARGEUR));
    t.push_str(&ligne("Total HT", &euro(panier.total_ht())));
    t.push_str(&ligne("Total TVA", &euro(panier.total_tva())));
    t.push_str(&ligne("TOTAL TTC", &euro(panier.total_ttc())));
    let _ = writeln!(t, "{}", "-".repeat(LARGEUR));

    t.push_str(&ligne(&format!("Paye ({moyen})"), &euro(paye)));
    let rendu = (paye - panier.total_ttc()).max(Decimal::ZERO);
    if rendu > Decimal::ZERO {
        t.push_str(&ligne("Rendu", &euro(rendu)));
    }
    let _ = writeln!(t, "{}", "-".repeat(LARGEUR));
    let _ = writeln!(t, "{}", centre("Merci de votre visite"));
    t
}

/// Centre un texte sur la largeur du ticket.
fn centre(s: &str) -> String {
    let texte: String = s.chars().take(LARGEUR).collect();
    let manque = LARGEUR.saturating_sub(texte.chars().count());
    let gauche = manque / 2;
    format!("{}{}", " ".repeat(gauche), texte)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ligne;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn entete() -> Entete {
        Entete {
            societe: "Pierre Angulaire".into(),
            adresse: "Charly".into(),
            siret: "12345678900011".into(),
            tva_intra: "FR12345678900".into(),
            numero: "A-0007".into(),
            date: "03/11/2026 16:40".into(),
            caisse: "1".into(),
        }
    }

    fn panier() -> Panier {
        let mut p = Panier::new();
        p.ajouter(Ligne::new("PAVE", "Pavé 14x14 gris", d("2"), d("19.99"), d("20")).unwrap())
            .unwrap();
        p.ajouter(Ligne::new("BORD", "Bordure T3", d("1"), d("9.99"), d("10")).unwrap())
            .unwrap();
        p
    }

    #[test]
    fn le_ticket_porte_les_mentions_obligatoires() {
        let t = ticket_texte(&entete(), &panier(), "especes", d("60"));
        assert!(t.contains("Pierre Angulaire"));
        assert!(t.contains("SIRET 12345678900011"));
        assert!(t.contains("TVA FR12345678900"));
        assert!(t.contains("Ticket A-0007"));
        assert!(t.contains("TOTAL TTC"));
    }

    #[test]
    fn le_recapitulatif_par_taux_additionne_les_bases() {
        let t = ticket_texte(&entete(), &panier(), "especes", d("60"));
        // Deux taux : 20 % sur 2 x 19,99 = 39,98 ; 10 % sur 9,99.
        assert!(t.contains("20 %"), "le taux 20 doit apparaitre : {t}");
        assert!(t.contains("10 %"), "le taux 10 doit apparaitre : {t}");
        // 39,98 + 9,99 = 49,97 HT
        assert!(t.contains("49,97"), "base HT totale attendue : {t}");
    }

    #[test]
    fn le_rendu_n_apparait_que_s_il_y_en_a_un() {
        let t = ticket_texte(&entete(), &panier(), "especes", d("60"));
        assert!(t.contains("Rendu"));
        let t2 = ticket_texte(&entete(), &panier(), "carte", d("53.96"));
        assert!(
            !t2.contains("Rendu"),
            "aucun rendu ne doit s'imprimer : {t2}"
        );
    }

    #[test]
    fn aucune_ligne_ne_depasse_la_largeur() {
        let mut p = Panier::new();
        // Un libelle tres long doit etre coupe, pas deborde.
        p.ajouter(
            Ligne::new(
                "X",
                "Un libelle absolument interminable qui depasse la largeur du papier",
                d("1"),
                d("1"),
                d("20"),
            )
            .unwrap(),
        )
        .unwrap();
        let t = ticket_texte(&entete(), &p, "carte", d("1.20"));
        for l in t.lines() {
            assert!(
                l.chars().count() <= LARGEUR,
                "ligne trop large ({} colonnes) : {l}",
                l.chars().count()
            );
        }
    }

    #[test]
    fn les_montants_s_ecrivent_a_la_francaise() {
        assert_eq!(euro(d("1234.5")), "1 234,50");
        assert_eq!(euro(d("0.05")), "0,05");
        assert_eq!(euro(d("999.999")), "1 000,00");
        assert_eq!(euro(d("-12.3")), "-12,30");
    }
}
