## Architecture

NV2 is a **native Rust voxel engine** with an MLP neural renderer, multiplayer, ECS gameplay, and a content pipeline. GPU-driven rendering via wgpu, chunked world with real-time meshing.

### Engine Pipeline

```mermaid
graph TB
    subgraph "Core"
        ECS["ECS Scheduler<br/>Bevy-inspired"]
        EVENTS["Event Bus<br/>pub/sub"]
        RES["Resource Manager<br/>Assets + state"]
    end

    subgraph "Render Pipeline"
        WGPU["wgpu Device<br/>GPU abstraction"]
        CHUNK_MESH["Chunk Mesher<br/>Greedy meshing"]
        NEURAL["MLP Neural Renderer<br/>Terrain approximation"]
        POST["Post-Processing<br/>Bloom + tone map"]
        SCREEN["Swap Chain<br/>Display output"]
    end

    subgraph "World"
        VOXEL["Voxel Storage<br/>Sparse octree"]
        GEN["World Generator<br/>Perlin + erosion"]
        PHYS["Physics<br/>AABB collision"]
    end

    subgraph "Network"
        PEER["Peer Manager<br/>WebRTC / MQTT"]
        SYNC["World Sync<br/>Delta compression"]
        STATE["State Reconcile<br/>Client prediction"]
    end

    subgraph "Game"
        INV["Inventory System"]
        CRAFT["Crafting Recipes"]
        UI["egui HUD<br/>Health / inventory"]
    end

    ECS --> CHUNK_MESH
    ECS --> NEURAL
    ECS --> VOXEL
    ECS --> PHYS
    ECS --> PEER
    ECS --> INV

    VOXEL -->|"chunk data"| CHUNK_MESH
    GEN -->|"populate"| VOXEL
    CHUNK_MESH -->|"vertex buffer"| WGPU
    NEURAL -->|"terrain map"| WGPU
    WGPU --> POST --> SCREEN
    PEER --> SYNC --> VOXEL
    INV --> UI

    style NEURAL fill:#F15A24,color:#000,stroke:none
    style WGPU fill:#1a1a2e,color:#F15A24,stroke:#F15A24
    style ECS fill:#1a1a2e,color:#DA2C38,stroke:#DA2C38
```

### Neural Renderer Detail

```mermaid
flowchart LR
    A["Chunk Heightmap<br/>32×32 f32"] --> B["Positional Encoding<br/>sin/cos frequency"]
    B --> C["MLP Layer 1<br/>128 → 64 ReLU"]
    C --> D["MLP Layer 2<br/>64 → 32 ReLU"]
    D --> E["Output Head<br/>32 → 4 (RGBA)"]
    E --> F["Terrain Texture<br/>Approximation"]

    G["Light Direction"] --> H["Diffuse + AO"]
    H --> E

    I["Training Data<br/>Voxel snapshots"] -->|"offline batch"| C
    I -->|"MSE loss"| J["Optimizer<br/>Adam lr=1e-3"]

    style NEURAL fill:#F15A24,color:#000,stroke:none
```

### Multiplayer Sync

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    participant W as World State

    C->>S: Input (position, action)
    S->>W: Validate + apply
    W->>S: Delta (changed chunks only)
    S->>C: State update (compressed)
    C->>C: Client-side prediction
    C->>S: Reconciliation (if mismatch)

    Note over C,S: Delta compression: only voxel changes sent, not full chunks
    Note over S,W: Server authoritative: all mutations go through server
```

## Quickstart

### One-liner — Build & run

```bash
git clone https://github.com/BartoszOsiej/NV2_ENGINE && cd NV2_ENGINE && cargo run --release
```

### One-liner — Run with debug rendering

```bash
git clone https://github.com/BartoszOsiej/NV2_ENGINE && cd NV2_ENGINE && NV2_DEBUG=1 cargo run --release
```

### One-liner — Docker (headless server mode)

```bash
docker run --rm -v "$(pwd)":/build -w /build \
  -e NV2_HEADLESS=1 \
  ghcr.io/bartoszosiej/nv2-ci:latest \
  cargo run --release -- --server --port 7777
```

### One-liner — Generate world + export

```bash
cd NV2_ENGINE && cargo run --release -- --generate --seed 42 --export world.nv2
```

### One-liner — Run benchmarks

```bash
cd NV2_ENGINE && cargo run --release -- --bench-mesh --bench-neural --bench-ecs 2>&1 | tee benchmarks.txt
```

### Verify

```bash
# Quick smoke test (builds + runs headless for 5 seconds)
cargo test && cargo run --release -- --headless --frames 300 --quit
```
