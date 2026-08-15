# NV-2.0 — Store Assets

## Capsules (generated placeholders)

`generate_capsules.py` (PIL) produces store-ready capsule art from the
game's own font + procedural voxel terrain. **These are placeholders** —
replace with final art before launch.

| File | Size | Store |
|---|---|---|
| `egs/key_art_3840x2160.png` | 3840×2160 | EGS key art |
| `egs/portrait_1024x1024.png` | 1024×1024 | EGS portrait |
| `egs/landscape_2560x1440.png` | 2560×1440 | EGS landscape |
| `egs/details_800x450.png` | 800×450 | EGS details |
| `itchio/header_630x500.png` | 630×500 | itch.io header |

Regenerate:
```bash
python3 generate_capsules.py
```

## Screenshots

Captured from real gameplay (via grim on the dev machine) and upscaled to
store minimums:

- `screenshots/screenshot_01_gameplay_1920x1080.png` / `_1280x720.png` — rainforest world (Amazon climate)
- `screenshots/screenshot_02_menu_1920x1080.png` / `_1280x720.png`
- `screenshots/screenshot_03_desert_1920x1080.png` / `_1280x720.png` — Sahara climate (dunes + oasis)
- `screenshots/screenshot_04_taiga_1920x1080.png` / `_1280x720.png` — Siberian climate (boreal forest)

Capture new ones with the engine's `--autostart` flag (boots straight into
a fresh world) and the `NV2_FPS_LOG=1` env var if needed.
