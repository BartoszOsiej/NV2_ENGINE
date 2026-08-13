# NV2.0 Engine — Test Report & QA

> Generated: 2026-08-13 · Rust `cargo 1.97.1` · Linux (Arch)
> Re-run everything with:
> ```bash
> cd Core
> cargo test                              # whole-project unit tests
> cargo test --release qa_benchmark_report -- --ignored --nocapture   # benchmark
> cargo clippy --all-targets              # lint / static analysis
> ```

## 1. Whole-project tests

**Result: ✅ 83 passed · 0 failed · 1 ignored** (the ignored one is the
release-only benchmark) — total 84 tests, 3.56 s.

## 2. Per-module tests

| Module | Tests | Status |
|---|---|---|
| `world::ai_generator` (AI system, checkpoints, procedural textures) | 9 | ✅ |
| `world::memplp` (**MeMLP** core — MLP train/forward, migration, JSON roundtrip) | 7 | ✅ |
| `world::block` (block registry) | 7 | ✅ |
| `world::vegetation` (AI vegetation placement, canopies, tree pass) | 3 | ✅ |
| `world::online_trainer` (internet datasets + offline fallback) | 2 | ✅ |
| `world::biomes` | 1 | ✅ |
| `world` (world manager, spawn, raycast, saves) | 6 | ✅ |
| `interaction` (block break/place incl. AI feedback) | 20 | ✅ |
| `crafting` | 6 | ✅ |
| `inventory` | 13 | ✅ |
| `renderer::camera` | 3 | ✅ |
| `renderer::mesh` (chunk meshing + water) | 2 | ✅ |
| `renderer::texture_registry` | 1 | ✅ |
| `commands` | 3 | ✅ |
| `assets` | 1 | ✅ |

## 3. Module categories

| Category | Modules | Tests | Status |
|---|---|---|---|
| **AI / ML** | ai_generator, memplp, online_trainer, vegetation, biomes | 22 | ✅ |
| **World & terrain** | block, world | 13 | ✅ |
| **Gameplay** | interaction, crafting, inventory | 39 | ✅ |
| **Renderer** | camera, mesh, texture_registry | 6 | ✅ |
| **Shell / misc** | commands, assets | 4 | ✅ |

## 4. Performance benchmark (release, single-threaded CPU)

Model: **MeMLP v1** — vegetation `8→24→16→4`, biome `8→12→9`,
texture `8→12→6` (623 params after migration, 1095 fresh).

### Inference

| Task | Latency | Throughput |
|---|---|---|
| Vegetation head forward | 292.9 ns | 3.41 M iter/s |
| Biome head forward | 334.9 ns | 2.99 M iter/s |
| Texture head forward | 290.6 ns | 3.44 M iter/s |
| 16×16 procedural texture tile | 28.1 µs | 35.6 k tiles/s |

### Training

| Task | Throughput |
|---|---|
| Vegetation head (backprop) | 504 k samples/s |
| Biome head (backprop) | 1.02 M samples/s |
| Texture head (backprop) | 1.16 M samples/s |

→ A whole epoch of 200 synthetic samples takes well under 1 ms; the AI can
train continuously in the background with zero visible frame cost.

## 5. Security & static analysis

| Check | Result |
|---|---|
| `unsafe` blocks in the whole codebase | **0** |
| Clippy (`cargo clippy --all-targets`) | 46 warnings, **0 errors** (mostly pre-existing style nits; new `memplp` module is warning-free) |
| Checkpoint input handling | JSON parse failures fall back to a fresh model (no panics on corrupt files) |
| Player-feedback buffer | bounded at 4096 samples (memory-safe) |
| Duplicate dependencies | 23 entries — all benign transitive aliases (wgpu build-deps) |

## 6. MeMLP upgrade notes

- New architecture: **MeMLP — Modular embedded Multi-layer Perceptron Model**
  (`Core/Src/world/memplp.rs`).
- Vegetation head deepened from `8→16→4` to `8→24→16→4`.
- Added biome head (`8→12→9`, matches `BiomeId`) and texture head
  (`8→12→6`) — both trained in the background loop and checkpointed.
- **Backward compatible:** old single-hidden-layer checkpoints are detected
  and migrated automatically (trained weights preserved); the shipped
  `Core/checkpoints/ai_model.json` was migrated to the v1 MeMLP format.
- Biome head is consumed by `DecorationAI` (biome-aware decoration choice).
