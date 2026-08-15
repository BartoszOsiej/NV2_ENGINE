# NV-2.0 — Store Publication Prep (itch.io + Epic Games)

**Status: release-ready.** Gameplay, real-climate world gen (NASA POWER),
Externum DRM, platform builds and the full EGS submission kit are done and
verified. This document is the go-live checklist for both stores.

---

## 1. What is ready

| Area | State |
|---|---|
| Gameplay (playable for hours) | Day/night, health/hunger/thirst, hostiles AI, tools+durability, 3×3 crafting, progression/achievements, menu, pause, save/load, respawn |
| World generation | **Real climate to the bone**: seed → Earth coordinates → embedded **NCEP 1981–2010 global climatology** (73×144 grid, `tools/fetch_climate.py` → `Core/assets/climate.dat`, bundled in the binary) + **NASA POWER live** refinement. Whittaker-style climate zones pick the dominant biome (Sahara seed → 92% desert, Amazon → 94% rainforest, Siberia → 57% taiga — verified by tests). Seasons drive HUD weather/temperature/clouds; regional atmosphere (sky+fog tint) follows the climate — a desert world is sun-baked, a rainforest humid, a taiga steel-blue. Walking north gets genuinely colder (lat/lon per column). Fixed the seed→lat mapping bug that made **every world arctic (58–70°N)** |
| Ground cover | AI vegetation rewritten: biome+surface-aware, clustered (no more random grey sticks/pebbles in deserts); debug `println!` spam + per-minute AI checkpoint writes removed |
| Live sky & weather | Fullscreen sky pass: climate-tinted gradient, **sun disc + glow, moon, stars, procedurally animated clouds** (coverage from the real cloud cover); **visible rain and snow particles** driven by the daily climate (rain in the Amazon, snow in a Siberian winter); sky/sun/terrain lighting follow the game clock (fixed renderer-vs-clock mismatch that made the world start at “midnight”) |
| Wildlife | **Voxel animals with procedural fur textures from the atlas** (deer in forests/taiga, rabbits on plains) — fixed the WGSL storage-buffer layout bug (vec3 16-byte alignment) that made every animal invisible, **walk-cycle animation** (opposing leg swing, body roll, faster gait when fleeing), and they **spawn in a cone in front of the camera** so wildlife is actually visible (previously they spawned anywhere in a ring — often behind the player). Verified on screen: deer in Siberia (1300+ px, moving), rabbit in London (ears visible) |
| Engine hardening | Saves + settings moved to per-user data dir (Windows `%LOCALAPPDATA%\NV2`, Linux `~/.local/share/nv2`) — **EGS/Program Files is read-only**, this was a real blocker; `--version`, `--autostart` and `--seed <n>` flags; 126 Rust tests green |
| Security (all in Externum `.ext`) | `tools/nv2_launcher.ext` — license gate + game-binary integrity + launch; `tools/nv2_installer.ext` — installer; `tools/build_nv2.ext` — build pipeline; `tools/keygen.ext` — key issuance; `lib/drm.ext` — HMAC sign/verify, key digest, file sha256 — all using the **unified base64 key format** (CLI `externum keygen` and Externum stdlib speak the same format, verified by cross-tests). Compiled with `--protect` → watermarked, tamper-checked artifacts. No hand-written `.py` in the protection layer |
| Externum stdlib | + `jsonx` (JSON load/dump) and `net` (HTTP GET — NASA POWER fetchable from Externum); 190 tests green |
| Engine hardening | Saves + settings moved to per-user data dir (Windows `%LOCALAPPDATA%\NV2`, Linux `~/.local/share/nv2`) — **EGS/Program Files is read-only**, this was a real blocker; `--version` and `--autostart` flags; 122 Rust tests green |
| Builds | Linux x86_64 release built + packaged (`release/nv2-engine-linux-x86_64.tar.gz`); **Windows x86_64 cross-built** (`Core/target/x86_64-pc-windows-gnu/release/nv2_engine.exe`, 34.3 MB) + staged in `EGS/build` |
| EGS submission kit | `EGS/` — BuildConfig.json, build_manifest.json (Externum-generated), PRIVACY_POLICY.txt, STORE_COPY.md (EN/PL), README.md (BuildPatchTool steps); `store_assets/` — capsule art (5 sizes, PIL) + real gameplay screenshots (1920×1080 + 1280×720); CI workflow `.github/workflows/windows-build.yml` |
| Tests | Rust **126 passed / 0 failed** (incl. realism guards: dominant biome matches real climate, embedded grid spot-checks Sahara/Amazon/Siberia); Externum **190 passed / 0 failed** |

Release artifacts live in `NV2_ENGINE/release/`:
`nv2_engine`, `nv2_launcher.ebin`, `nv2.key` (dev key), `README.txt`,
`EULA.txt`, `nv2-engine-linux-x86_64.tar.gz`.

> **Rebuild note (DRM secret):** the shipped `nv2_launcher.ebin` /
> `nv2_installer.ebin` embed the sha256 of the *current* `nv2_engine`
> binary, so they must be re-issued after any engine change. This needs
> the private `<DRM-SECRET>` (never committed). See section 2.

---

## 2. Rebuilding & issuing player keys

> **Good news — EGS needs no key at all.** The game is **keyless on Epic**: the
> storefront is the ownership gate, `LaunchCommand` is `nv2_engine.exe`
> directly, and the binary has no license check inside it (verified: no DRM
> references in `main.rs`). The DRM launcher (`nv2_launcher.ebin`) is only for
> itch.io / direct-sale builds. It also auto-switches to keyless when Epic
> launches it (`-EpicPortal` / `-EpicApp`).
>
> A fresh DRM secret was generated locally (it was lost) and stored at:
> **`~/.nv2-drm-secret`** (chmod 600). The `release/` artifacts + `dist/` were
> rebuilt with it and verified end-to-end: good key runs, bad key rejected,
> tampered binary rejected, keyless + Epic mode run. If you ever rebuild,
> reuse that file — or generate a new one and rebuild `release/`.

```bash
# build + DRM-protect (from projects/externum) — runs with the real secret
python3 -m externum run tools/build_nv2.ext -- \
  --secret "$(cat ~/.nv2-drm-secret)" --target linux --core ../NV2_ENGINE/Core

# Windows (same pipeline, produces the .exe used by EGS)
python3 -m externum run tools/build_nv2.ext -- \
  --secret "$(cat ~/.nv2-drm-secret)" --target windows --core ../NV2_ENGINE/Core

# issue player license keys (one per sold copy)
python3 -m externum run tools/keygen.ext -- \
  --secret "$(cat ~/.nv2-drm-secret)" --app-id nv2-engine --author "NV-2.0" --count 50

# regenerate the EGS manifest after any rebuild
python3 -m externum run tools/egs_manifest.ext -- \
  ../NV2_ENGINE/EGS/build ../NV2_ENGINE/EGS/build_manifest.json
```

Keep `~/.nv2-drm-secret` private. It is **never** shipped — artifacts embed
only the digest of a valid key, so a stolen game copy cannot mint new keys.

---

## 3. itch.io

### 3.1 Page setup (https://itch.io/game/new)
- **Title:** NV-2.0
- **Short description:** *Voxel survival with a real climate — every world
  maps to a location on Earth and pulls its weather from NASA.*
- **Classification:** Game
- **Kind of project:** Desktop
- **Genre tags:** Voxel, Sandbox, Survival, Open World, Simulation
- **Tags:** voxel, survival, crafting, open-world, procedural, climate,
  nasa, sandbox, exploration, singleplayer
- **Pricing:** see section 5.
- **Release status:** Released
- **Uploads (Butler):** Linux tar.gz + Windows zip.
- **Links:** repository → this project; website → none yet.

### 3.2 Butler upload
```bash
butler push NV2_ENGINE/release/nv2-engine-linux-x86_64.tar.gz \
  <user>/nv2-0:linux-x86_64 --userversion 1.0.0
```

### 3.3 itch.io page copy (EN / PL)

> **EN — short:** Every world in NV-2.0 is a real place. The seed picks a
> spot on Earth and NASA POWER feeds its actual climate into the terrain,
> the weather and the sky. Mine, craft, build, survive the night — and
> never trust a rainy forecast.
>
> **PL — short:** Każdy świat w NV-2.0 to prawdziwe miejsce. Seed wybiera
> punkt na Ziemi, a NASA POWER zasila generację rzeczywistym klimatem —
> biomami, pogodą i niebem. Kop, twórz, buduj i przetrwaj noc.

### 3.4 itch.io requirements checklist
- [x] Header/capsule image 630×500 (`store_assets/itchio/header_630x500.png`)
- [x] 2+ screenshots (`store_assets/screenshots/`)
- [ ] Cover image for search (use header or a crop)
- [ ] Choose payment (sales) — account must be verified
- [ ] Set base price (section 5)
- [ ] Upload Linux build + Windows zip + verify launch
- [ ] Set visibility → Public
- [ ] Fill credits/team, license (custom EULA), privacy (`EGS/PRIVACY_POLICY.txt`)

---

## 4. Epic Games Store

### 4.1 Pre-requisites
- EGS developer account → [dev.epicgames.com](https://dev.epicgames.com)
- Company/individual verification (tax + payout forms) — **starts the
  clock, do it first**
- Game page approval + store review (typically 2–6 weeks)

### 4.2 Submission steps
1. **Product setup:** create product "NV-2.0", genre (Sandbox,
   Simulation), rating (PEGI 3 / ESRB E — no blood, no gore), age gate.
2. **Store presence:** title, description (`EGS/STORE_COPY.md`), capsule
   images (`store_assets/egs/` — 3840×2160 key art, 1024×1024 portrait,
   2560×1440 landscape, 800×450 details), screenshots
   (`store_assets/screenshots/`), tags.
3. **Build configuration:** `EGS/BuildConfig.json` + `EGS/build/` +
   `EGS/build_manifest.json`; run BuildPatchTool ChunkData (steps in
   `EGS/README.md`). First executable = `nv2_engine.exe` (keyless — the
   storefront is the ownership gate). The DRM launcher stays for
   itch/direct builds.
4. **Entitlements:** keyless delivery (recommended) or key-based via
   `tools/keygen.ext` uploaded to EGS.
5. **Review checklist:** crash-free on target, no debug builds, EULA,
   privacy policy (`EGS/PRIVACY_POLICY.txt`), support email, refund policy.
6. **Release:** set price (section 5), submit for certification.

### 4.3 EGS page copy (EN / PL)
Same short copy as itch.io (section 3.3) + long description with features:
real NASA POWER climate per world, AI-driven terrain, day/night survival,
3×3 crafting, hostiles, achievements, save/load, offline fallback
(`EGS/STORE_COPY.md`).

---

## 5. Pricing proposal

### Base price: **$9.99 (≈ 42 PLN)**

Rationale:
- Genre anchor: voxel survival/sandbox (Minecraft-class) usually sits
  $19.99–29.99. NV-2.0 is a solo/indie title with a smaller content
  surface — undercutting to $9.99 is an honest launch price for an indie.
- Launch promotion: **$7.49 (-25%)** for the first 2 weeks to build
  reviews/momentum; the 10% itch.io + 12% EGS fee cut lands ≈ $6.7/sale
  net at launch price.

### Platform settings
| Store | Price | Fee | Notes |
|---|---|---|---|
| itch.io | $9.99 (min. $4.99 slider) | 10% | "Pay what you want" OFF; slider min. $4.99 |
| Epic Games Store | $9.99 | 12% | keyless (recommended) or key-based |

### Regional notes
- PL storefront: 42 PLN flat on itch.io; EGS regionalizes automatically.
- Windows build is **done** (`EGS/build/nv2_engine.exe`) and reproducible
  via CI (`.github/workflows/windows-build.yml`).

---

## 6. Go-live checklist
- [x] Capsule art + screenshots generated (`store_assets/`)
- [x] Windows build produced + staged (`EGS/build/`)
- [x] EGS submission kit (config, manifest, privacy, store copy)
- [x] CI workflow for repeatable Windows builds
- [ ] Rebuild release artifacts with the real `<DRM-SECRET>` after final
      engine changes (section 2)
- [ ] itch.io page live (Linux build + price)
- [ ] Windows zip pushed to both stores
- [ ] 50 launch keys generated for reviewers/influencers
- [ ] EGS account verification started (tax/payout)
- [ ] EGS submission + certification
- [ ] Release announcement (post on itch + socials)

**Security reminder:** never commit `github-token.md` or the DRM secret.
The secret lives only on the build machine (and in your password manager).
