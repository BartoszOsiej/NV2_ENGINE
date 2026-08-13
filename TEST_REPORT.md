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

**Result: ✅ 96 passed · 0 failed · 1 ignored** (the ignored one is the
release-only benchmark) — total 97 tests, ~15 s.

> Phase-2 features added during the 2026-08-13 sweep: community model
> sharing (export/import), JSON training-dataset import, player-preference
> learning, and the `/ai_*` command set — each with tests.

## 2. Per-module tests

| Module | Tests | Status |
|---|---|---|
| `world::ai_generator` (AI system, model bundles, datasets, preferences, checkpoints, textures) | 16 | ✅ |
| `world::memplp` (**MeMLP** core — MLP train/forward, NaN hardening, migration, JSON roundtrip) | 10 | ✅ |
| `world::block` (block registry) | 7 | ✅ |
| `world::vegetation` (AI vegetation placement, canopies, tree pass) | 3 | ✅ |
| `world::online_trainer` (internet datasets + offline fallback) | 2 | ✅ |
| `world::biomes` | 1 | ✅ |
| `world` (world manager, spawn, raycast, saves) | 6 | ✅ |
| `interaction` (block break/place incl. AI feedback) | 19 | ✅ |
| `crafting` | 6 | ✅ |
| `inventory` | 13 | ✅ |
| `renderer::camera` | 3 | ✅ |
| `renderer::mesh` (chunk meshing + water) | 2 | ✅ |
| `renderer::texture_registry` | 1 | ✅ |
| `commands` (locate/tp + `/ai_export`, `/ai_import`, `/ai_dataset`, `/ai_stats`) | 6 | ✅ |
| `assets` | 1 | ✅ |

## 3. Module categories

| Category | Modules | Tests | Status |
|---|---|---|---|
| **AI / ML** | ai_generator, memplp, online_trainer, vegetation, biomes | 32 | ✅ |
| **World & terrain** | block, world | 13 | ✅ |
| **Gameplay** | interaction, crafting, inventory | 38 | ✅ |
| **Renderer** | camera, mesh, texture_registry | 6 | ✅ |
| **Shell / misc** | commands, assets | 7 | ✅ |

## 4. Performance benchmark (release, single-threaded CPU)

Model: **MeMLP v1** — vegetation `8→24→16→4`, biome `8→12→9`,
texture `8→12→6` (623 params after migration, 1095 fresh).

> Numbers below are from one machine and depend on CPU load — treat them as
> a floor, not a spec. Re-run the benchmark for your own hardware.

### Inference

| Task | Latency | Throughput |
|---|---|---|
| Vegetation head forward | 693.5 ns | 1.44 M iter/s |
| Biome head forward | 332.3 ns | 3.01 M iter/s |
| Texture head forward | 298.4 ns | 3.35 M iter/s |
| 16×16 procedural texture tile | 27.2 µs | 36.8 k tiles/s |

### Training

| Task | Throughput |
|---|---|
| Vegetation head (backprop) | 453 k samples/s |
| Biome head (backprop) | 894 k samples/s |
| Texture head (backprop) | 962 k samples/s |

→ A whole epoch of 200 synthetic samples takes well under 1 ms; the AI can
train continuously in the background with zero visible frame cost.

## 5. Security & static analysis

| Check | Result |
|---|---|
| `unsafe` blocks in the whole codebase | **0** |
| Clippy (`cargo clippy --all-targets`) | 43 warnings, **0 errors** (mostly pre-existing style nits; new `memplp` hardening is warning-free) |
| Checkpoint input handling | JSON parse failures fall back to a fresh model (no panics on corrupt files) |
| NaN/Inf checkpoint poisoning | blocked at 3 layers: gradient clipping, sanitise-on-save, `null`→`0.0` on load |
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

## 7. Phase-2 features (this sweep)

### Community model sharing — `ModelBundle`

Portable, self-describing export format (`nv2-model-bundle`, v1) that wraps
a full checkpoint with authorship metadata (author, description, biome
hint, export timestamp). In-game: `/ai_export <path> [author]` and
`/ai_import <path>`; API: `AISystem::export_model` / `import_model`.
Imports are sanitised and persisted to the runtime checkpoint.

### Training datasets — `TrainingDataset`

JSON files with `samples` (8 terrain features) + `targets` (4-class
vegetation distributions) can be trained on directly. Validation rejects
empty/mismatched files and skips non-finite rows. In-game:
`/ai_dataset <path> [epochs]`; API: `AISystem::train_on_dataset`.

### Player-preference learning

`TerrainAI` now tracks per-class preference counters (flower/fern/stick/
pebble) in the checkpoint (`#[serde(default)]` — old checkpoints stay
compatible). Placing a vegetation block increments its counter; the
background loop blends heuristic targets with the learned distribution
(30% weight), so the model leans toward what the player likes.
`/ai_stats` shows the live counters.

## 8. NaN hardening (this sweep)

A real bug found by the QA sweep: background training could explode weights
into NaN (unbounded gradient updates). serde_json serialises NaN as JSON
`null`, which made the whole checkpoint unloadable (`load_checkpoint`
returned `None` and the trained model was silently discarded).

Fixed in three layers:

1. **`Mlp::train`** — input/target validated (NaN/Inf samples skipped),
   gradients clipped to ±5, per-parameter updates bounded to ±1, and a
   non-finite loss triggers a full weight sanitise instead of propagating.
2. **`save_checkpoint`** — sanitises a clone (NaN/Inf → 0.0) before writing,
   so the file on disk can never contain `null` weights.
3. **`load_checkpoint`** — tolerant: JSON `null` weights read back as `0.0`,
   so even a previously-poisoned checkpoint loads instead of being discarded.

Regression tests: `training_survives_extreme_inputs_without_nan`,
`training_skips_nan_inputs_without_touching_weights`, `sanitize_clears_nan_and_inf`
(`memplp.rs`) and `poisoned_checkpoint_with_null_weights_still_loads`
(`ai_generator.rs`).

Phase-2 tests: `model_bundle_export_import_roundtrip`,
`model_bundle_export_matches_live_model`, `import_rejects_non_bundle_files`,
`train_on_dataset_imports_and_reduces_loss`,
`dataset_validation_rejects_bad_files`, `player_preferences_shift_training_targets`
(`ai_generator.rs`) and `ai_export_import_commands_roundtrip`,
`ai_stats_command_reports_live_model`, `ai_commands_validate_usage`
(`commands.rs`).
