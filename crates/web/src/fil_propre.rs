//! Chaque requete s'execute sur son propre fil d'execution.
//!
//! Les handlers sont `async` parce qu'axum l'exige, mais **aucun n'`await` sur
//! une entree-sortie** : chaque appel `db::...` est un aller-retour MySQL
//! bloquant, et chaque rendu fait du calcul. Tant qu'ils tournent sur un fil du
//! reacteur, une requete lente (tableau de bord, export, PDF) immobilise ce
//! worker pour tout le monde ; quand tous les workers sont pris, le serveur ne
//! lit meme plus les sockets.
//!
//! On reproduit donc le modele de PHP, celui auquel les utilisateurs de
//! Dolibarr sont habitues : une requete, un fil. Le reacteur ne fait plus que
//! lire la socket et rendre la reponse. Le cout est la creation d'un fil par
//! requete (quelques dizaines de microsecondes), paye une fois pour un echange
//! qui dure des millisecondes a plusieurs secondes.
//!
//! Le nombre de fils simultanes est plafonne : au-dela, la requete attend son
//! tour au lieu de faire grossir le processus sans limite.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Semaphore};
use tower::ServiceExt;

/// Requetes simultanees autorisees. Genereux pour une equipe, et assez bas
/// pour qu'un pic ne transforme pas la machine en tas de fils.
const MAX_SIMULTANEES: usize = 64;

/// Reponse de secours quand le fil n'a pas pu rendre la sienne.
fn panne() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Body::from("le serveur n'a pas pu traiter la demande"))
        .expect("reponse de secours")
}

/// Sert le routeur, une requete par fil.
///
/// Le routeur est place en `fallback_service` d'un routeur vide : c'est la
/// seule facon de faire passer *toutes* les requetes par le meme service sans
/// toucher a une seule route.
pub(crate) async fn servir(listener: TcpListener, routes: Router) {
    let permis = Arc::new(Semaphore::new(MAX_SIMULTANEES));

    let service = tower::service_fn(move |requete: Request<Body>| {
        let routes = routes.clone();
        let permis = permis.clone();
        async move {
            // Le permis est rendu a la fin du fil, pas a la fin de la lecture :
            // c'est le travail, pas l'attente, qu'on plafonne.
            let permis = match permis.acquire_owned().await {
                Ok(p) => p,
                // Le semaphore ne se ferme jamais : si cela arrivait, refuser
                // proprement vaut mieux que paniquer dans le reacteur.
                Err(_) => return Ok(panne()),
            };

            let (tx, rx) = oneshot::channel();
            std::thread::spawn(move || {
                // Un runtime minimal, propre a ce fil : `block_on` y est legal
                // (on n'est pas dans un contexte asynchrone) et le SQL ne
                // bloque que ce fil.
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(_) => {
                        let _ = tx.send(panne());
                        return;
                    }
                };
                let reponse = rt.block_on(async move {
                    match routes.oneshot(requete).await {
                        Ok(r) => r,
                        // Le routeur est infaillible ; cette branche existe pour
                        // que le type soit complet, pas parce qu'elle sert.
                        Err(_) => panne(),
                    }
                });
                // Le permis est relache ici : `drop` explicite pour que la fin
                // de la section se lise sans connaitre Rust.
                drop(permis);
                let _ = tx.send(reponse);
            });

            match rx.await {
                Ok(r) => Ok(r),
                // Le fil est mort avant de repondre (panique dans un handler).
                Err(_) => Ok(panne()),
            }
        }
    });

    // Le routeur vide avec un seul `fallback` : toute requete, quelle que soit
    // son adresse, passe par le service ci-dessus.
    let app = Router::new().fallback_service(service);
    axum::serve(listener, app).await.expect("serveur");
}
