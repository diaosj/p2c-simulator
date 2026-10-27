# p2c-simulator

A small load-balancing lab: ~5,000 requests per second routed across 300 rate-limited upstream nodes, with the routing strategy switchable at runtime. It exists to make one point visible: **power-of-two-choices (P2C) gets almost all of the benefit of "pick the globally best node" at O(1) cost, and without a shared view of the cluster.**

The scenario is the one you meet behind any LLM or SaaS gateway: every upstream (an API key, a vendor account, a replica) has a quota; when it is exhausted it returns `429` until the window resets. The router's job is to spend the quota evenly so the fleet as a whole produces as few `429`s as possible.

## What it simulates

- **300 nodes**, each with a quota of 100 requests. A node whose quota hits zero is *exhausted* and answers `429` until it is healed.
- **Traffic**: 50 requests every 10 ms (~5,000 QPS), generated inside the backend.
- **Healer**: every 2 s all quotas reset to 100. So the fleet can absorb 15,000 requests per window, and 10,000 arrive; a good router produces zero `429`s, a bad one burns hot nodes while cold ones sit idle.
- **Strategies** (`POST /api/strategy` with `Random`, `GlobalLeast` or `P2C`):
  - `Random`: uniform choice. No state, no fairness.
  - `GlobalLeast`: scan all 300 nodes and pick the one with the fewest `429`s so far. The best decision, at O(n) per request, and it needs a consistent global view.
  - `P2C`: sample two distinct nodes at random, send to the one with fewer `429`s. O(1), no global view, and provably close to the optimum (Mitzenmacher, *The Power of Two Choices in Randomized Load Balancing*, 2001).

The backend exposes `GET /api/state` (all nodes, current strategy, total requests, total `429`s). The front end polls it once a second and paints the 300 nodes as a grid: green = has quota, red = exhausted. Switch strategies and watch how much of the grid turns red.

## Run it

Backend (Rust, axum):

```bash
cd backend
cargo run            # listens on http://0.0.0.0:3000
cargo test           # unit tests for the three routing strategies
```

Front end (React 19, Vite, Tailwind):

```bash
cd frontend
npm install
npm run dev          # opens on http://localhost:5173, polls the backend at :3000
```

Switch strategy while it runs:

```bash
curl -X POST localhost:3000/api/strategy -d P2C
curl -X POST localhost:3000/api/strategy -d GlobalLeast
curl -X POST localhost:3000/api/strategy -d Random
curl localhost:3000/api/state | jq '{strategy, total_requests, total_429s}'
```

## Why I built it

I ran an AI-platform gateway for five years and now run the model-routing layer of an agent product. Every time the question "least-connections or something cheaper?" came up, the answer was P2C, and every time someone wanted to see it rather than take it on faith. This is the thing to show them.

## Status

V1. The routing core and the API are done and tested; the front end is a plain grid. CORS is permissive because it is a local demo. Ideas for V2: per-strategy `429` rate charted over time, skewed node capacities, and a `least_conn` strategy with in-flight counters, which is the variant most gateways actually ship.
