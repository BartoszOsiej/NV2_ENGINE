# 🛡️ Asset Audit — third-party content check

> Date: 2026-08-14 · Scope: `NV2_ENGINE/Assets/` + `NV2_ENGINE/Bridge/`

## Result: third-party assets WERE present and have been removed

The repository shipped with a large set of assets that were **not original** —
they are byte-for-byte rips of **Mojang / Minecraft** vanilla content, which
is proprietary and **must not be distributed** in a game release:

| Location | Files | What it was |
|---|---|---|
| `Assets/Blocks/` | 1695 PNG | Vanilla Minecraft block textures (`oak_log.png`, `crafter_east.png`, `grass_block_top.png`, `piston_top_sticky.png`, …) — names match Minecraft 1.21+ exactly |
| `Assets/Models/Block/` | 578 JSON | Vanilla Minecraft block models (cube/cube_all/cube_column parents, `minecraft:` texture refs) |
| `Assets/Recipes/` | 1471 JSON | Vanilla Minecraft crafting recipes (`mojang_banner_pattern.json`, smithing/trim templates, …) |
| `Assets/Atlas/` | 5 PNG | Ripped texture atlases (`terrain.png`, `rudy.png`, `trawa_kamien.png`, …) |

Also found: `Assets/Fonts/Subtitles/Doto-VariableFont_ROND,wght.ttf` — a
Google Fonts typeface licensed under the **SIL Open Font License 1.1**, which
**permits redistribution** with attribution. Kept — see `ATTRIBUTION.md`.

## Remediation (per decision: remove and replace with procedural)

1. All Mojang-derived files (textures, models, recipes, atlas rips) were
   **deleted** from the repository.
2. **Textures** — the engine already ships a deterministic procedural tile
   generator (`Core/Src/world/ai_generator.rs`):
   `texture_style_for_name()` maps every atlas slot to an original palette +
   pattern (wood, leaves, stone, ore speckle, water, lava, …) and
   `generate_tile_texture()` draws a 16×16 tile from a name-seeded hash. The
   atlas (`Core/Src/renderer/texture_atlas.rs`) now composes **100%**
   procedurally — the same world on every machine, zero external files.
3. **Block models** — the model loader degrades gracefully
   (`unwrap_or_default()`), and blocks fall back to atlas-tile rendering.
4. **Recipes** — the JSON recipe pack was not loaded by the runtime at all
   (crafting uses the in-code registry in `Core/Src/crafting.rs`). Removed.
5. **Bridge/Tools/Slicer** — a Windows-only C# tool whose only purpose was
   slicing an external atlas into per-block PNGs (i.e. importing rips). It is
   now obsolete; the engine no longer consumes `Assets/Blocks` PNGs.

## Keeping the build green

- `Assets/Blocks/`, `Assets/Models/Block/`, `Assets/Recipes/` are kept as
  empty directories (`.gitkeep`) — the atlas composer and model loader probe
  for them and handle absence gracefully.
- Verified with `cargo check` + `cargo test` (see `TEST_REPORT.md`).

## Guidance

- Do **not** re-add Mojang asset files (textures, models, recipes, sounds,
  fonts, names) to this repository. The block *names* in `Core` (`oak_log`,
  `grass`, …) are generic voxel-game terms and stay — only Mojang's actual
  files were removed.
- If hand-made textures are added later, drop them into `Assets/Blocks/` and
  the atlas composer will pick them up automatically.
