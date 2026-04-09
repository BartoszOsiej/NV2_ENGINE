# NV_ENGINE MVP - Integration & Setup Guide

## Current Status

✅ **Completed Systems:**
1. AABB Player Physics with Collision Detection
2. Infinite World Generation with Chunk Management
3. Face Culling Optimization for Mesh Generation
4. DDA Raycasting System
5. Block Setting/Getting Infrastructure
6. JSON-based Asset Loading System
7. Camera Controls (90° FOV, pitch clamping)
8. Chunk Unloading for Memory Management

## File Structure

```
Core/
├── Src/
│   ├── main.rs                 (event loop, initialization)
│   ├── input.rs                (input state management)
│   ├── assets.rs              (JSON asset loaders) ✨ NEW
│   ├── renderer/
│   │   ├── mod.rs             (State, RENDER_RADIUS, update)
│   │   ├── camera.rs          (Camera, AABB physics)
│   │   ├── mesh.rs            (ChunkMesh with face culling)
│   │   ├── vertices.rs
│   │   ├── texture_atlas.rs
│   │   ├── texture.rs
│   │   └── Instance.rs
│   └── world/
│       ├── mod.rs             (World, infinite loading)
│       ├── block.rs           (BlockType enum)
│       ├── chunk.rs           (Chunk with set/get)
│       ├── biomes.rs
│       └── raycast.rs         (DDA raycasting) ✨ NEW
├── Cargo.toml                  (updated with serde, serde_json, rayon)
└── Assets/
    ├── Blocks/                 (PNG textures)
    ├── Models/Block/          (JSON block models) ✨ NEW
    └── Recipes/               (JSON recipes) ✨ NEW
```

## Usage Examples

### 1. Loading Block Models

```rust
use crate::assets::BlockModelLoader;

// In main.rs or initialization:
let block_models = BlockModelLoader::load_all("Assets/Models/Block/")
    .expect("Failed to load block models");

// Access a model:
if let Some(grass_model) = block_models.get("grass") {
    println!("Grass textures: {:?}", grass_model.textures);
}
```

### 2. Loading & Validating Recipes

```rust
use crate::assets::RecipeManager;

let recipes = RecipeManager::load_all("Assets/Recipes/")
    .expect("Failed to load recipes");

// Validate shapeless recipe:
if let Some(Recipe::Shapeless { .. }) = recipes.get("wooden_planks") {
    let items = vec!["oak_log".to_string()];
    let valid = RecipeManager::validate_shapeless(&recipe, &items);
}

// Validate shaped (3x3) recipe:
let grid = [[None, None, None],
            [None, None, None],
            [None, None, None]];
let valid = RecipeManager::validate_shaped(&recipe, &grid);
```

### 3. Using Raycasting for Block Targeting

```rust
use crate::world::raycast;

// In renderer update or input handling:
let hit = raycast(
    camera.position,           // ray origin
    camera.get_forward(),       // ray direction  
    5.0,                        // max distance
    &world
);

if let Some(hit) = hit {
    println!("Hit block at {:?}, face {}", hit.block_pos, hit.face);
    // hit.face: 0=top, 1=bottom, 2=front, 3=back, 4=right, 5=left
    // hit.distance: distance from camera to block center
}
```

### 4. Block Placement/Breaking

```rust
// Breaking blocks (left-click):
if let Some(hit) = raycast(camera.position, camera.get_forward(), 5.0, &world) {
    world.set_block(
        hit.block_pos.0,
        hit.block_pos.1,
        hit.block_pos.2,
        BlockType::Air
    );
    
    // Remesh affected chunks
    let cx = hit.block_pos.0 / 16;
    let cz = hit.block_pos.2 / 16;
    // TODO: invalidate GPU chunk meshes for re-generation
}

// Placing blocks (right-click):
if let Some(hit) = raycast(camera.position, camera.get_forward(), 5.0, &world) {
    let (bx, by, bz) = hit.block_pos;
    let (nx, ny, nz) = match hit.face {
        0 => (bx, by + 1, bz),    // place above
        1 => (bx, by - 1, bz),    // place below
        2 => (bx, bz + 1, bz),    // place forward
        3 => (bx, by, bz - 1),    // place back
        4 => (bx + 1, by, bz),    // place right
        5 => (bx - 1, by, bz),    // place left
        _ => return,
    };
    
    world.set_block(nx, ny, nz, BlockType::Grass);
}
```

## Performance Notes for i7-4610M + Radeon HD 8700M

### Current Optimizations:
- ✅ Face culling (reduces vertex count by ~80%)
- ✅ Chunk-based rendering (only renders loaded chunks)
- ✅ Per-axis collision (reduces collision checks)

### Recommended Further Optimizations:
1. **Chunk Meshing in Background Threads** (priority)
   ```rust
   use rayon::prelude::*;
   
   // Parallel mesh generation for multiple chunks
   let meshes: HashMap<_, _> = chunks
       .par_iter()
       .map(|(coords, chunk)| {
           (*coords, ChunkMesh::generate(&world, coords.0, coords.1))
       })
       .collect();
   ```

2. **Mesh Caching**
   - Store GPU mesh until block modified
   - Only regenerate affected chunks

3. **LOD (Level of Detail)**
   - Simple mesh for distant chunks
   - Full detail for nearby chunks

4. **Frustum Culling**
   - Skip rendering chunks outside camera view

5. **Block Update Batching**
   - Queue block changes
   - Regenerate meshes in batch

## JSON Schema Reference

### Block Model (JSON)
```json
{
  "name": "block_name",
  "textures": [
    "texture_top",
    "texture_bottom", 
    "texture_front",
    "texture_back",
    "texture_right",
    "texture_left"
  ],
  "opaque": true,
  "breakable": true
}
```

### Recipe - Shapeless (JSON)
```json
{
  "name": "recipe_name",
  "type": "shapeless",
  "ingredients": ["item1", "item2"],
  "result": {
    "item": "result_item",
    "count": 1
  }
}
```

### Recipe - Shaped (JSON)
```json
{
  "name": "recipe_name", 
  "type": "shaped",
  "pattern": [
    "XXX",
    "XYX",
    "XXX"
  ],
  "key": {
    "X": "oak_planks",
    "Y": "stick"
  },
  "result": {
    "item": "crafting_table",
    "count": 1
  }
}
```

## Next Implementation Tasks

### Priority 1: Block Interaction UI
- [ ] Render selection box around targeted block
- [ ] Handle mouse clicks for block breaking/placing
- [ ] Visual feedback on placement attempts

### Priority 2: Chunk Mesh Invalidation
- [ ] Track which chunks need remeshing
- [ ] Regenerate meshes in background
- [ ] Update GPU buffers asynchronously

### Priority 3: Advanced Rendering
- [ ] Implement frustum culling
- [ ] Add LOD system
- [ ] Shadows and better lighting

### Priority 4: Game Systems
- [ ] Inventory system
- [ ] Crafting UI
- [ ] Block variants (oak vs birch, etc.)

## Compilation & Testing

```bash
# Check compilation
cd Core
cargo check

# Run with logging
RUST_LOG=debug cargo run

# Build release
cargo build --release
```

## Known Limitations

1. **No selection wireframe yet** - Raycasting works but UI not integrated
2. **Chunk remeshing** - Mesh not updated when blocks change
3. **No inventory system** - Can't store or select blocks
4. **Single player only** - No networking
5. **No crafting UI** - Recipe system loaded but not integrated

## Architecture Decisions

### AABB Collision
- Per-axis resolution prevents getting stuck in corners
- Smooth sliding when walking into walls at angles

### Chunk Size: 16x256x16
- Manageable memory footprint
- Good balance for meshing performance
- Standard Minecraft chunk dimensions

### DDA Raycasting
- More accurate than voxel walking
- Detects exact face hit
- Configurable step distance

### Face Culling
- Significant memory/bandwidth savings
- Check neighbor opacity before rendering face
- Works with transparent blocks (water)

## Support & Debugging

Enable detailed logging:
```rust
// In main.rs
env_logger::builder()
    .filter_level(log::LevelFilter::Debug)
    .init();
```

Check chunk loading:
```rust
println!("Chunks loaded: {}", world.chunks.len());
```

Monitor frame time:
```rust
let fps = 1.0 / dt;
println!("FPS: {:.1}", fps);
```

---

**Last Updated:** March 26, 2026
**Status:** MVP Core Systems Implemented
**Ready for:** Block Interaction & UI Integration
