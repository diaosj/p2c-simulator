use axum::{extract::State, routing::get, Json, Router};
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

#[derive(Clone, Serialize, Deserialize)]
struct Node {
    id: usize,
    is_exhausted: bool,
}

type SharedState = Arc<RwLock<Vec<Node>>>;

async fn get_state(State(state): State<SharedState>) -> Json<Vec<Node>> {
    let nodes = state.read().await;
    Json(nodes.clone())
}

#[tokio::main]
async fn main() {
    let nodes: Vec<Node> = (0..50)
        .map(|id| Node {
            id,
            is_exhausted: false,
        })
        .collect();

    let shared_state: SharedState = Arc::new(RwLock::new(nodes));

    // Background task: simulate 429 errors and recoveries every second
    let state_clone = shared_state.clone();
    tokio::spawn(async move {
        let mut rng = StdRng::from_entropy();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let mut nodes = state_clone.write().await;

            // Pick 5 random nodes and set them as exhausted
            let indices: Vec<usize> = (0..nodes.len()).collect();
            let exhausted_picks: Vec<usize> = indices
                .choose_multiple(&mut rng, 5)
                .cloned()
                .collect();
            for &i in &exhausted_picks {
                nodes[i].is_exhausted = true;
            }

            // Pick 3 random exhausted nodes and recover them
            let exhausted_indices: Vec<usize> = nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.is_exhausted)
                .map(|(i, _)| i)
                .collect();
            let recovery_picks: Vec<usize> = exhausted_indices
                .choose_multiple(&mut rng, 3)
                .cloned()
                .collect();
            for &i in &recovery_picks {
                nodes[i].is_exhausted = false;
            }
        }
    });

    let app = Router::new()
        .route("/api/state", get(get_state))
        .with_state(shared_state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Backend listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

