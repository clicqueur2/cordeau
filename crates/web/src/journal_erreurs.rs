//! Journal des erreurs : fichier + ecran.
//!
//! Une erreur qui ne laisse pas de trace est une erreur qui reviendra. Chaque
//! erreur servie au navigateur ecrit une ligne : date, chemin, message.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Dossier des journaux. `DOLIBARR_LOGS` sinon `./logs`.
pub fn dossier() -> PathBuf {
    std::env::var("DOLIBARR_LOGS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("logs"))
}

pub fn fichier() -> PathBuf {
    dossier().join("erreurs.log")
}

/// Ajoute une ligne au journal. Ne panique jamais : si le disque refuse, on
/// continue (une erreur de journal ne doit pas casser une reponse).
pub fn enregistrer(chemin: &str, message: &str) {
    let _ = fs::create_dir_all(dossier());
    let ligne = format!("{} | {chemin} | {message}\n", horodatage());
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(fichier()) {
        let _ = f.write_all(ligne.as_bytes());
    }
}

/// Dernieres lignes du journal, les plus recentes en dernier.
pub fn dernieres(max: usize) -> Vec<String> {
    let contenu = fs::read_to_string(fichier()).unwrap_or_default();
    let lignes: Vec<&str> = contenu.lines().collect();
    let debut = lignes.len().saturating_sub(max);
    lignes[debut..].iter().map(|s| s.to_string()).collect()
}

/// Nombre de lignes du journal.
pub fn compter() -> usize {
    fs::read_to_string(fichier())
        .map(|c| c.lines().count())
        .unwrap_or(0)
}

/// Date lisible, sans dependance : on emprunte l'arithmetique de `db`.
fn horodatage() -> String {
    let maintenant = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let jours = maintenant.div_euclid(86_400);
    let reste = maintenant.rem_euclid(86_400);
    let (annee, mois, jour) = db::reglement::civil_depuis_jours_public(jours);
    format!(
        "{annee:04}-{mois:02}-{jour:02} {:02}:{:02}:{:02}",
        reste / 3600,
        (reste % 3600) / 60,
        reste % 60
    )
}
