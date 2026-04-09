# NV_ENGINE Technologies and Current Implementation

Date: 2026-04-09

## 1. Solution overview

NV_ENGINE is a native desktop voxel-engine project focused on real-time world rendering, procedural terrain generation, interaction, inventory/crafting gameplay, and supporting content-pipeline tools. The current repository is not just a terrain prototype anymore. It now contains a working gameplay loop with menus, commands, save/load, item handling, block interaction, world simulation, and GPU-driven rendering.

At a high level, the solution is organized into:

- A Rust runtime in `Core/Src` that contains the engine, gameplay, renderer, world simulation, and UI logic.
- Auxiliary content tools in `Bridge/Tools` built on .NET for atlas slicing and related asset preparation.
- Supporting resource and packaging files in `Assets`, `.vscode`, and `VulkanLayers`.

## 2. Technologies used in the solution

### 2.1 Core language and platform

- Rust 2021 is the primary implementation language for the engine and gameplay runtime.
- The project targets desktop execution on Windows and is currently packaged/run as a native executable from the `Core` crate.
- C# on .NET 8 is used for supporting bridge/content-pipeline tools under `Bridge/Tools`.
- Python is used for lightweight texture manipulation utilities in `generate_textures.py`.

### 2.2 Rendering, windowing, math, and GPU data

- `wgpu = 0.20`
  Used as the rendering backend abstraction. The renderer manages the surface, device, queue, swapchain configuration, multiple render pipelines, GPU buffers, texture bind groups, and depth targets.

- `winit = 0.30`
  Used for the application/event loop, window lifecycle, raw input events, cursor capture, and keyboard/mouse routing.

- `cgmath = 0.18`
  Used for vectors, matrices, camera transforms, ray directions, AABB math, and general 3D coordinate operations.

- `bytemuck`
  Used for safe POD-style conversion when uploading vertex, index, and uniform data to GPU buffers.

### 2.3 Assets, text, and data serialization

- `image = 0.25`
  Used for texture loading, atlas composition, and image processing inside the Rust asset pipeline.

- `fontdue = 0.7`
  Used for font loading, glyph rasterization, text measurement, and text rendering preparation.

- `serde` and `serde_json`
  Used for world serialization, JSON-driven block models, recipe parsing, and persistent state storage.

- `anyhow`
  Used for error propagation and contextual failure reporting in loading, saving, and asset operations.

### 2.4 Terrain generation, concurrency, and diagnostics

- `opensimplex2 = 1.1.0`
  Used for procedural terrain, climate, cave, ore, and vegetation sampling.

- `rayon = 1`
  Used to parallelize chunk generation through batched async dispatch onto the global thread pool.

- `num_cpus = 1`
  Present to support CPU-aware scaling/configuration decisions around generation work.

- `env_logger` and `log`
  Used for runtime logging and diagnostics.

### 2.5 Auxiliary tools and packaging components

- `.NET 8` plus `System.Drawing.Common`
  Used in the bridge/slicer tools for atlas inspection and PNG extraction workflows.

- `Pillow` in `generate_textures.py`
  Used for rotation, flipping, brightness changes, grayscale/invert, and related texture transformations.

- Custom Vulkan layer packaging
  The repository includes `VkLayer_NV20.json` and `VkLayer_NV20.dll`, allowing the engine to be launched with a custom Vulkan layer in the existing VS Code task configuration.

## 3. Current technical implementation

### 3.1 Application shell and runtime flow

The application entry point is `Core/Src/main.rs`. It uses `winit::application::ApplicationHandler` and manages three explicit runtime modes:

- `MainMenu`
- `Playing`
- `PauseMenu`

The `App` struct currently owns:

- The renderer state.
- The `World` instance.
- Input accumulation state.
- Save/load path handling.
- Status/subtitle messaging.
- Slash-command input state.
- Main menu and pause menu selection state.

Current runtime behaviors implemented in the shell:

- New game flow.
- Save and load flow.
- Pause/resume transitions.
- Cursor capture toggling between gameplay and GUI/command entry.
- Slash-command prompt opened with `/`.
- Command execution routed through `commands::execute(...)`.
- On-screen feedback passed into the renderer subtitle/command prompt system.

### 3.2 Rendering architecture

Rendering is centered in `Core/Src/renderer/mod.rs` through `renderer::State`.

The renderer currently owns and manages:

- WGPU surface/device/queue/configuration.
- Camera uniforms and material/biome uniforms.
- A depth texture.
- A texture atlas and texture bind groups.
- Separate pipelines for:
  - solid world geometry
  - water geometry
  - flat UI panels
  - sprite-based UI icons
  - text rendering
- A menu renderer and text renderer.
- An `InteractionController` that bridges gameplay interaction and GUI state.

Important renderer-side implementation details currently in place:

- Chunk meshes and water meshes are cached separately by chunk coordinate.
- Mesh creation is rate-limited so only a small number of chunks are built per frame.
- GPU uploads are debounced instead of re-uploading buffers every time a single chunk changes.
- Water mesh recombination is separated from full water mesh rebuilds.
- Seam repair is performed when new chunks arrive so neighboring chunk borders are rebuilt correctly.
- CPU frustum culling was intentionally removed from packed uploads in favor of conservative submission near the player.

The renderer also drives several gameplay-visible systems:

- Day/night phase progression through `elapsed_time`.
- Water animation timing.
- Climate/biome-driven fog and ambient color uniforms.
- Crosshair rendering.
- Subtitle overlays.
- Command prompt overlays.
- Main menu and pause menu rendering.
- Inventory and crafter panel rendering.

### 3.3 Text and UI implementation

UI is a hybrid of:

- Procedural flat panel quads.
- Sprite-based item icons derived from the atlas.
- Fontdue-backed text layers.

Current UI systems include:

- Main menu.
- Pause menu.
- Hotbar.
- Full inventory overlay.
- Player 2x2 crafting overlay.
- NVCrafter 3x3 crafting overlay.
- Hovered slot detection and visual state.
- Command prompt text rendering.
- Subtitle/status messaging.

The menu system is implemented in `renderer/menu.rs`, and font rasterization plus text measurement/render preparation are implemented in `renderer/text.rs`.

### 3.4 Camera, movement, and collision

Player/camera movement lives in `Core/Src/renderer/camera.rs`. There is no separate player-controller module. The authoritative movement path is:

- event/input capture in `main.rs`
- `renderer::State::update(...)`
- `Camera::tick_movement(...)`

Currently implemented movement features:

- First-person camera rotation.
- Walking and sprinting.
- Jumping.
- Flight toggle.
- Gravity and fall-speed limiting.
- Water-specific gravity and sinking behavior.
- AABB-based collision against solid blocks.
- Input-intent capture separated from movement integration.
- Runtime movement modifiers sampled from overlapping block mediums.

The block system exposes movement-medium metadata through `BlockType::movement_medium(...)`, and the camera already tracks `in_foliage_medium` and `footstep_volume` for future audio/gameplay hooks.

One important current boundary is that there is still no dedicated gameplay audio system consuming those movement-medium signals.

### 3.5 World representation and chunk streaming

The main world container is `Core/Src/world/mod.rs`.

`World` currently stores:

- Loaded chunks in a `(cx, cz) -> Chunk` map.
- A shared `BiomeGenerator`.
- A `ChunkGenerator` for background generation.
- A receiver for completed chunk messages.
- Pending chunk tracking.
- Pending cross-chunk world writes.
- Tree-population tracking for already-processed chunks.
- Per-block `NVCrafterState` entries.
- Buffered `WorldItemDrop` entities.

Current world features include:

- Synchronous near-player chunk materialization.
- Background generation of more distant chunks.
- Unloading of far chunks with a buffer radius.
- Cross-chunk world-write buffering.
- Lazy chunk generation when cross-border block mutations require a destination chunk.
- Block read/write by world coordinates.
- Numeric block ID read/write helpers.
- Block placement and destruction helpers.
- Runtime item-drop buffering and draining.
- Save/load support.
- Safe teleport position resolution.
- Spawn search based on real block occupancy and runtime clearance.

### 3.6 Asynchronous chunk generation

The async generation layer is implemented in `Core/Src/world/generator.rs`.

The current generator design is:

- A bounded queue of chunk coordinates.
- A separate in-flight set to deduplicate dispatch.
- Batched flushing from the main thread each frame.
- Parallel chunk generation through `rayon::spawn(...)` plus `into_par_iter()`.
- Result delivery back to the main thread via `mpsc`.

This means chunk generation is already decoupled from rendering and gameplay updates, while still keeping world insertion and mesh rebuild coordination on the main thread.

### 3.7 Procedural terrain and biome system

Terrain and climate generation are implemented in `Core/Src/world/biomes.rs` and related world-generation helpers.

The current biome model includes these biome IDs:

- Ocean
- Coast
- Plains
- Forest
- DarkForest
- Swamp
- Taiga
- Desert
- Mountains

The generator currently combines multiple OpenSimplex-derived channels and dedicated seeds for:

- continent shape
- temperature
- humidity
- erosion
- peaks/relief
- height/detail
- warp/surface variation
- caves
- ores
- water

The world generator also exposes visual/climate data back to the renderer through ambient color, fog color, fog density, scene grade, vegetation tint, warmth, moisture, and lushness values.

Based on the repository structure and current generator integration, the terrain system already supports:

- biome-specific surface and ground selection
- cave carving
- ore placement
- water-aware column sampling
- vegetation and tree placement
- climate-driven visual variation

### 3.8 Vegetation and world-space tree propagation

Tree and vegetation logic lives in `Core/Src/world/vegetation.rs` and uses both terrain-time planning and a post-insert world pass.

Important current behaviors:

- Terrain can emit deferred world writes.
- Once a chunk exists in `World`, the explicit world-space tree pass can run.
- `World::populate_world_trees_for_chunk(...)` ensures that post-insert vegetation is applied only once per explicitly inserted chunk.
- Cross-chunk canopy writes are supported.
- Destination chunks created only because of canopy spillover remain terrain-only until explicitly loaded/populated.

The vegetation implementation currently includes:

- trunk planning before canopy placement
- biome-specific canopy shapes
- support checks for terrain, grass, flower, and shrub surfaces
- deterministic variation through seed-based randomization
- explicit world-space validation for trunk/canopy placement

### 3.9 Block model and texture systems

There are two asset-facing systems relevant here:

- `assets.rs` for block models and JSON recipes.
- `renderer/texture_registry.rs` plus `renderer/texture_atlas.rs` for runtime texture resolution.

Current implementation in this area includes:

- JSON block model loading.
- Normalization of block model textures into a stable 6-face array.
- Thread-local caching of normalized block models.
- JSON recipe loading and validation utilities.
- Atlas-based UV lookup for block textures.
- Runtime texture discovery from `Assets/Blocks`.
- Top/bottom/side texture variant handling.
- Fallback-friendly texture name normalization.
- Texture pack switching support in the renderer.
- Subtitle font discovery/copy support through `ensure_subtitle_font()`.

### 3.10 Liquid simulation

Dynamic liquid behavior is implemented in `Core/Src/world/liquid.rs` with world integration in `world/mod.rs` and separate rendering in `renderer/mesh.rs`.

The liquid system currently uses:

- Per-voxel `water_meta` storage in each chunk.
- Level encoding where `0` means no dynamic state, `1..7` means flow, and `8` means source.
- A gravity-first solver.
- Lateral spread when downward flow is blocked.
- A hard cap on block changes per simulation step.

The renderer already treats water as a distinct mesh/pipeline path, and `renderer::State::update(...)` drives simulation on a timer before triggering throttled mesh updates.

### 3.11 Interaction, mining, placement, and drops

Player-world interaction is implemented in `Core/Src/interaction.rs`.

Current interaction features include:

- DDA raycast block targeting via `world::raycast`.
- Persistent break-progress state for held left-click mining.
- Right-click interaction routing.
- Block placement using the active inventory stack.
- Prevention of placing blocks inside the player AABB.
- Tool-gated harvesting through block hardness, required tool tier, and tool power.
- Tool durability consumption on valid harvest actions.
- Context-specific drop logic.

Special cases already implemented:

- Gravel can drop flint via deterministic seeded drop logic.
- Destroying a tree trunk can harvest the connected trunk/leaves cluster instead of only a single block.
- Leaf destruction can additionally drop saplings and sticks using deterministic odds.
- Breaking an `NVCrafter` flushes its stored contents into world drops before the block is removed.

### 3.12 Inventory, item stacks, and crafting

Inventory logic lives in `Core/Src/inventory.rs`, while the crafting system lives in `Core/Src/crafting.rs`.

Current inventory implementation includes:

- 36-slot player inventory.
- 9-slot hotbar mapped to the tail of the inventory.
- Active hotbar selection and mouse-wheel scrolling.
- Stack merging and overflow handling.
- Tool durability tracking.
- Separation between placeable items and inventory-only items.
- Built-in 2x2 player crafting grid and output slot.

Current crafting implementation includes:

- `CraftingGrid` with fixed 9-slot backing and variable width/height.
- `RecipeRegistry` with shaped and shapeless recipes.
- `NVCrafterState` for world-backed 3x3 crafting stations.
- Pattern matching with offset support for shaped recipes.
- Multiset matching for shapeless recipes.

Default recipes currently present in code include:

- logs -> planks
- planks -> sticks
- planks ring with early-game log in the center -> NVCrafter
- wooden pickaxe
- wooden axe
- wooden shovel
- wooden hoe
- torches
- chest
- door
- trapdoor
- ladder
- fence
- fence gate
- workbench upgrade
- flint pickaxe
- stone pickaxe
- iron pickaxe

### 3.13 GUI-aware interaction layer

The interaction controller also acts as the GUI transaction layer between the player inventory and the world.

This layer currently handles:

- opening and closing the inventory GUI
- opening an NVCrafter GUI when the targeted block supports it
- slot hover detection
- drag-and-drop between slot types
- output-slot click handling
- returning crafting inputs when a GUI is closed
- moving NVCrafter inputs back to the player inventory, or dropping them into the world if needed

UI slot addressing is explicit through `UiSlotId`, which distinguishes between:

- player inventory slots
- player crafting input slots
- player crafting output
- NVCrafter input slots
- NVCrafter output

### 3.14 Commands, save/load, and player feedback

Command handling is implemented in `Core/Src/commands.rs`.

Commands currently implemented:

- `/locate <biome> [--tp]`
- `/tp <x> <y> <z>`

Current command behavior:

- `/locate` samples chunk rings outward from the player, using fixed offsets per chunk.
- Results can optionally teleport the player using `World::safe_teleport_position(...)`.
- Errors and command responses are shown both through stdout and the in-engine subtitle/command prompt system.

Save/load is implemented in `world/mod.rs` and currently serializes:

- world seed
- flattened chunk block data
- chunk water metadata
- persisted NVCrafter states

### 3.15 Supporting tools in the repository

The repository contains non-runtime support tooling that is already part of the overall solution:

- `Bridge/Tools/Slicer/Program.cs`
  A .NET-based atlas slicing utility that scans atlas PNGs and extracts block textures using either predefined or analyzed tile rectangles.

- `generate_textures.py`
  A Pillow-based texture transformation script used to rotate, flip, grayscale, invert, darken, or brighten source textures.

- `.vscode/tasks.json`
  A task is already configured to run the engine executable from the workspace.

- `VulkanLayers/VkLayer_NV20.json` and `VkLayer_NV20.dll`
  These package a custom Vulkan layer configuration alongside the engine.

## 4. Current status summary

At the current repository state, the solution already provides:

- a native Rust desktop application
- WGPU-based 3D rendering and UI rendering
- async chunk generation with bounded work dispatch
- procedural terrain, climate, caves, ores, water, and vegetation
- world-space tree propagation across chunk borders
- block placement, destruction, harvesting, and drops
- dynamic liquid simulation
- inventory and hotbar management
- 2x2 player crafting and 3x3 world crafter support
- GUI drag-and-drop between gameplay slot types
- slash commands for locate and teleport
- world save/load
- menu, pause, subtitle, and command prompt UI
- asset helpers for textures, atlas composition, block models, and recipes
- auxiliary .NET and Python tooling for asset preparation

The main technical areas that are clearly not implemented as first-class subsystems yet are:

- dedicated gameplay audio
- networking or multiplayer
- an editor or in-engine content authoring tool
- a formal ECS-style gameplay architecture

In other words, the repository already represents a playable and technically layered voxel-engine/gameplay prototype, not only a rendering or terrain-generation experiment.