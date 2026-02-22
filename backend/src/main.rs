use axum::{extract::State, routing::{get, post}, Json, Router};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

#[derive(Clone, Serialize, Deserialize)]
struct Node {
    id: usize,
    is_exhausted: bool,
    quota: i32,
    error_429_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
enum Strategy {
    Random,
    GlobalLeast,
    P2C,
}

struct AppState {
    nodes: Arc<RwLock<Vec<Node>>>,
    current_strategy: Arc<RwLock<Strategy>>,
    total_requests: Arc<AtomicUsize>,
    total_429s: Arc<AtomicUsize>,
}

impl AppState {
    fn route_request(&self, nodes: &[Node]) -> usize {
        let mut rng = rand::thread_rng();
        let len = nodes.len();

        match *self.current_strategy.try_read().unwrap_or_else(|e| {
            panic!("Strategy RwLock poisoned in route_request: {}", e)
        }) {
            Strategy::Random => rng.gen_range(0..len),
            Strategy::GlobalLeast => {
                nodes
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, n)| n.error_429_count)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
            Strategy::P2C => {
                let a = rng.gen_range(0..len);
                let b = if len > 1 {
                    (a + rng.gen_range(1..len)) % len
                } else {
                    a
                };
                if nodes[a].error_429_count <= nodes[b].error_429_count {
                    a
                } else {
                    b
                }
            }
        }
    }
}

#[derive(Serialize)]
struct StateResponse {
    nodes: Vec<Node>,
    strategy: Strategy,
    total_requests: usize,
    total_429s: usize,
}

async fn get_state(State(state): State<Arc<AppState>>) -> Json<StateResponse> {
    let nodes = state.nodes.read().await.clone();
    let strategy = state.current_strategy.read().await.clone();
    let total_requests = state.total_requests.load(Ordering::Relaxed);
    let total_429s = state.total_429s.load(Ordering::Relaxed);
    Json(StateResponse {
        nodes,
        strategy,
        total_requests,
        total_429s,
    })
}

async fn set_strategy(State(state): State<Arc<AppState>>, body: String) -> String {
    let new_strategy = match body.trim() {
        "Random" => Strategy::Random,
        "GlobalLeast" => Strategy::GlobalLeast,
        "P2C" => Strategy::P2C,
        other => return format!("Unknown strategy: {}", other),
    };
    let mut strategy = state.current_strategy.write().await;
    *strategy = new_strategy;
    "OK".to_string()
}

#[tokio::main]
async fn main() {
    let nodes: Vec<Node> = (0..300)
        .map(|id| Node {
            id,
            is_exhausted: false,
            quota: 100,
            error_429_count: 0,
        })
        .collect();

    let app_state = Arc::new(AppState {
        nodes: Arc::new(RwLock::new(nodes)),
        current_strategy: Arc::new(RwLock::new(Strategy::P2C)),
        total_requests: Arc::new(AtomicUsize::new(0)),
        total_429s: Arc::new(AtomicUsize::new(0)),
    });

    // Background task: Simulator – generates ~5000 QPS traffic
    let sim_state = app_state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            // Spawn 50 concurrent requests per tick
            for _ in 0..50 {
                let st = sim_state.clone();
                tokio::spawn(async move {
                    let idx = {
                        let nodes = st.nodes.read().await;
                        st.route_request(&nodes)
                    };
                    st.total_requests.fetch_add(1, Ordering::Relaxed);
                    let mut nodes = st.nodes.write().await;
                    if nodes[idx].is_exhausted {
                        st.total_429s.fetch_add(1, Ordering::Relaxed);
                        nodes[idx].error_429_count += 1;
                    } else {
                        nodes[idx].quota -= 1;
                        if nodes[idx].quota <= 0 {
                            st.total_429s.fetch_add(1, Ordering::Relaxed);
                            nodes[idx].error_429_count += 1;
                            nodes[idx].is_exhausted = true;
                        }
                    }
                });
            }
        }
    });

    // Background task: Healer – resets all nodes every 2 seconds
    let heal_state = app_state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            let mut nodes = heal_state.nodes.write().await;
            for node in nodes.iter_mut() {
                node.quota = 100;
                node.is_exhausted = false;
            }
        }
    });

    let app = Router::new()
        .route("/api/state", get(get_state))
        .route("/api/strategy", post(set_strategy))
        .with_state(app_state)
        // TODO: Restrict CORS in production; this permissive configuration is intended for local development only.
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Backend listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_nodes(count: usize) -> Vec<Node> {
        (0..count)
            .map(|id| Node {
                id,
                is_exhausted: false,
                quota: 100,
                error_429_count: 0,
            })
            .collect()
    }

    fn make_app_state(nodes: Vec<Node>, strategy: Strategy) -> Arc<AppState> {
        Arc::new(AppState {
            nodes: Arc::new(RwLock::new(nodes)),
            current_strategy: Arc::new(RwLock::new(strategy)),
            total_requests: Arc::new(AtomicUsize::new(0)),
            total_429s: Arc::new(AtomicUsize::new(0)),
        })
    }

    #[test]
    fn test_node_defaults() {
        let node = Node {
            id: 0,
            is_exhausted: false,
            quota: 100,
            error_429_count: 0,
        };
        assert_eq!(node.quota, 100);
        assert_eq!(node.error_429_count, 0);
        assert!(!node.is_exhausted);
    }

    #[test]
    fn test_strategy_serde() {
        let s = Strategy::P2C;
        let json = serde_json::to_string(&s).unwrap();
        let deserialized: Strategy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Strategy::P2C);
    }

    #[test]
    fn test_route_request_random() {
        let nodes = make_test_nodes(300);
        let state = make_app_state(nodes.clone(), Strategy::Random);
        let idx = state.route_request(&nodes);
        assert!(idx < 300);
    }

    #[test]
    fn test_route_request_global_least() {
        let mut nodes = make_test_nodes(300);
        nodes[42].error_429_count = 0;
        for (i, node) in nodes.iter_mut().enumerate() {
            if i != 42 {
                node.error_429_count = 10;
            }
        }
        let state = make_app_state(nodes.clone(), Strategy::GlobalLeast);
        let idx = state.route_request(&nodes);
        assert_eq!(idx, 42);
    }

    #[test]
    fn test_route_request_p2c() {
        let mut nodes = make_test_nodes(2);
        nodes[0].error_429_count = 100;
        nodes[1].error_429_count = 0;
        let state = make_app_state(nodes.clone(), Strategy::P2C);
        // With only 2 nodes, P2C always picks both and should return index 1
        let idx = state.route_request(&nodes);
        assert_eq!(idx, 1);
    }

    #[tokio::test]
    async fn test_set_strategy_valid() {
        let nodes = make_test_nodes(10);
        let state = make_app_state(nodes, Strategy::P2C);
        {
            let mut s = state.current_strategy.write().await;
            *s = Strategy::Random;
        }
        let s = state.current_strategy.read().await;
        assert_eq!(*s, Strategy::Random);
    }
}

