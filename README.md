# 🎮 NV_ENGINE

**A native desktop voxel engine with AI-powered terrain generation.**

NV_ENGINE is a from-scratch voxel engine written in Rust — real-time world
rendering, procedural terrain generation, interaction, inventory/crafting
gameplay, and supporting content-pipeline tools. Beyond the terrain prototype,
the repo contains a working gameplay loop: menus, commands, save/load, item
handling, block interaction, world simulation, and GPU-driven rendering.

```
┌─────────────────────────────────────────────────────────────┐
│  NV_ENGINE                                                   │
│  ├── Core/        Rust runtime — engine, gameplay, renderer, │
│  │                world simulation, UI logic (wgpu, winit)   │
│  ├── Bridge/      .NET 8 content tools — atlas slicing and   │
│  │                asset preparation                           │
│  ├── Assets/      Resources and packaging                    │
│  └── VulkanLayers/ Supporting Vulkan layers                  │
└─────────────────────────────────────────────────────────────┘
```

## Quick start

```bash
cd Core
cargo run --release
```

What you'll see:

- The engine boots with no startup lag
- The world generates with AI-powered vegetation
- Flowers, ferns, pebbles placed intelligently by the neural network
- The AI learns in the background (no FPS impact)

## 🤖 AI — MeMLP (Modular embedded Multi-layer Perceptron Model)

The entire AI stack runs on **MeMLP** — a modular, embedded neural network
that lives inside the engine (pure CPU, JSON checkpoints, no cloud, no GPU):

```
Input: 8 terrain features (height, slope, biome temperature, humidity,
       water distance, nearby vegetation, light, noise)
    ↓
Vegetation head (deep MLP 8 → 24 → 16 → 4, ReLU + softmax)
    ↓
Softmax (4 outputs: flower / fern / stick / pebble)
    ↓
Placement gated by confidence + biome-aware decorations (biome head)
```

Modules — one checkpoint file, implemented in `Core/Src/world/memplp.rs`:

| Module | Shape | Task |
|---|---|---|
| `vegetation` | 8 → 24 → 16 → 4 | flower / fern / stick / pebble placement |
| `biome` | 8 → 12 → 9 | biome classification (9 biomes, drives decorations) |
| `texture` | 8 → 12 → 6 | procedural texture-style selection |

- **22 new vegetation types** — roses, tulips, daisies, water plants, moss,
  sticks, pebbles and more
- **Learns from the player** — placing/breaking vegetation is recorded as
  training feedback (`Core/Src/world/ai_feedback.rs`)
- **Online learning** — real-world climate data (Open-Meteo) merged into
  training with a graceful offline fallback
- **Backward compatible** — old single-hidden-layer checkpoints are
  detected and migrated automatically on load
- **Tiny footprint** — ~1 KB checkpoint, ~0.3 µs per prediction, millions of
  training samples/s on the background thread (see `TEST_REPORT.md`)

> Reproducibility: `cargo test --release qa_benchmark_report -- --ignored --nocapture`
> re-runs the full MeMLP benchmark.

## 🧱 Gameplay

- Block interaction (place / mine)
- Inventory and crafting
- Commands, menus, save / load
- Chunk-based world simulation

## 🤖 NV2.0 Phase-2 features

- **Community model sharing** — `/ai_export <path> [author]` writes a
  portable `nv2-model-bundle` (checkpoint + author/description/biome
  metadata); `/ai_import <path>` loads any shared bundle and persists it.
- **Training datasets** — `/ai_dataset <path> [epochs]` trains the
  vegetation head on a JSON dataset (`samples` + `targets`) with full
  validation.
- **Player-preference learning** — the model tracks which vegetation the
  player likes placing (counters live in the checkpoint) and blends its
  training targets toward that taste; `/ai_stats` shows live stats and
  preferences.

Commands: `/locate`, `/tp`, `/ai_export`, `/ai_import`, `/ai_dataset`,
`/ai_stats`.

## 🛠️ Tech stack

| Area | Technology |
|---|---|
| Engine & runtime | Rust (2021), wgpu 0.20, winit 0.30 |
| Content pipeline | C# / .NET 8 (`Bridge/Tools`) |
| Texture utilities | Python (`generate_textures.py`) |
| Rendering | GPU-driven renderer, Vulkan layers |

## 📚 Documentation

| File | Content |
|---|---|
| `TECHNOLOGIES_AND_CURRENT_IMPLEMENTATION.md` | Solution overview and tech stack |
| `AI_IMPLEMENTATION_SUMMARY.md` | AI vegetation system — implementation summary |
| `AI_TECHNICAL_DOCS.md` | Neural network mathematics and implementation details |
| `AI_PHASE2_ROADMAP.md` | Future plans (internet integration, GPU textures, …) |
| `QUICKSTART.md` | Build & run instructions |
| `CHANGELOG.md` | What changed |

## 🚀 Roadmap (Phase 2)

- [ ] Downloading training datasets
- [ ] GPU texture generation
- [ ] Real-time terrain editing
- [ ] Community model sharing
- [ ] Player preference learning

Details in `AI_PHASE2_ROADMAP.md`.
