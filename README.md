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

## 🤖 AI vegetation system

A lightweight MLP (multi-layer perceptron) trained in the background decides
where vegetation should grow:

```
Input: 8 terrain features (height, slope, biome temperature, humidity,
       water distance, nearby vegetation, light, noise)
    ↓
ReLU (16 neurons)
    ↓
Softmax (4 outputs: flower / fern / stick / pebble)
    ↓
Placement gated by biome (Forest 70%, Swamp 50%, …) and confidence
```

- **22 new vegetation types** — roses, tulips, daisies, water plants, moss,
  sticks, pebbles and more
- **Online learning** — 100 samples per epoch, ~5–10 ms per epoch, adaptive
  learning-rate decay
- **Tiny footprint** — 320 parameters, 1.2 KB model, ~0.01 ms per prediction

## 🧱 Gameplay

- Block interaction (place / mine)
- Inventory and crafting
- Commands, menus, save / load
- Chunk-based world simulation

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
