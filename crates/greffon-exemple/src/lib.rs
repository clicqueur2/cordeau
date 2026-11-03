//! Greffon d'exemple : la preuve que la frontiere C ABI fonctionne.
//!
//! Compile en bibliotheque dynamique, il exporte les trois symboles du contrat
//! et repond a un evenement en renvoyant son JSON enrichi. Un vrai greffon
//! ferait autre chose — pousser une facture vers un service, calculer une
//! remise, journaliser dans un outils externe — mais le mecanisme est le meme.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Version d'API du contrat. Le noyau refuse tout autre nombre.
#[no_mangle]
pub extern "C" fn dolibarr_api_version() -> u32 {
    1
}

/// Recoit un evenement JSON, rend une reponse JSON.
///
/// La chaine rendue est allouee ici : c'est `dolibarr_liberer` qui la reprend.
///
/// # Safety
///
/// `evenement` doit etre un pointeur valide vers une chaine C terminee par
/// un octet nul, ou nul. La chaine rendue doit etre rendue a
/// `dolibarr_liberer` exactement une fois.
#[no_mangle]
pub unsafe extern "C" fn dolibarr_greffon(evenement: *const c_char) -> *mut c_char {
    if evenement.is_null() {
        return std::ptr::null_mut();
    }
    let entree = CStr::from_ptr(evenement).to_string_lossy().to_string();

    // On ne fait pas de parsing JSON ici : un greffon n'a pas a dependre de la
    // bibliotheque du noyau. On renvoie ce qu'on a recu, plus notre marque.
    let reference = entree
        .split('"')
        .skip_while(|s| *s != "reference")
        .nth(2)
        .unwrap_or("inconnue");

    let reponse = format!(
        "{{\"ok\":true,\"greffon\":\"exemple\",\"reference\":\"{reference}\",\"message\":\"evenement traite\"}}"
    );
    CString::new(reponse)
        .map(|c| c.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Reprend la chaine rendue par `dolibarr_greffon`.
///
/// # Safety
///
/// `chaine` doit venir de `dolibarr_greffon` et n'avoir jamais ete liberee.
#[no_mangle]
pub unsafe extern "C" fn dolibarr_liberer(chaine: *mut c_char) {
    if chaine.is_null() {
        return;
    }
    let _ = CString::from_raw(chaine);
}
