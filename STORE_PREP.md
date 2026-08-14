# NV-2.0 — Store Publication Prep (itch.io + Epic Games)

**Status: release-ready.** Gameplay, real-climate world gen (NASA POWER),
Externum DRM and platform builds are done and verified. This document is
the go-live checklist for both stores.

---

## 1. What is ready

| Area | State |
|---|---|
| Gameplay (playable for hours) | Day/night, health/hunger, hostiles AI, tools+durability, 3×3 crafting, progression/achievements, menu, pause, save/load, respawn |
| World generation | Seed → Earth coordinates → **NASA POWER API** real climate (temp/precip/humidity/solar/wind) shapes biomes + weather + sky; deterministic offline fallback |
| Security (all in Externum `.ext`) | `tools/nv2_launcher.ext` — license gate + game-binary integrity + launch; compiled with `--protect` → watermarked, tamper-checked artifact. `lib/drm.ext` — HMAC sign/verify, key digest, file sha256. No hand-written `.py` anywhere in the protection layer |
| Builds | Linux x86_64 release built + packaged (`release/nv2-engine-linux-x86_64.tar.gz`); Windows build scripted (`--target windows`) |
| Tests | Rust **119 passed / 0 failed**; Externum **180 passed / 0 failed** |

Release artifacts live in `NV2_ENGINE/release/`:
`nv2_engine`, `nv2_launcher.ebin`, `nv2.key` (dev key), `README.txt`, `EULA.txt`, `nv2-engine-linux-x86_64.tar.gz`.

---

## 2. Rebuilding & issuing player keys

```bash
# build + DRM-protect (from projects/externum)
python3 -m externum run tools/build_nv2.ext -- \
  --secret <DRM-SECRET> --target linux --core ../NV2_ENGINE/Core

# Windows (needs rustup + mingw target on the build machine)
python3 -m externum run tools/build_nv2.ext -- \
  --secret <DRM-SECRET> --target windows --core ../NV2_ENGINE/Core

# issue player license keys (one per sold copy)
python3 -m externum keygen --app-id nv2-engine --author "NV-2.0" \
  --secret <DRM-SECRET> --count 50
```

Keep `<DRM-SECRET>` private. It is **never** shipped — artifacts embed only
the digest of a valid key, so a stolen game copy cannot mint new keys.

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
- **Uploads (Butler):** Linux tar.gz. Windows build when the cross-build
  is run on a rustup machine.
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
- [ ] Header/capsule image 630×500 (or 315×250)
- [ ] 5+ screenshots (1280×720 min)
- [ ] Cover image for search
- [ ] Choose payment (sales) — account must be verified
- [ ] Set base price (section 5)
- [ ] Upload Linux build + verify launch
- [ ] Set visibility → Public
- [ ] Fill credits/team, license (custom EULA), privacy

---

## 4. Epic Games Store

### 4.1 Pre-requisites
- EGS developer account → [publish.unrealengine.com / dev.epicgames.com](https://dev.epicgames.com)
- Company/individual verification (tax + payout forms) — **starts the
  clock, do it first**
- Game page approval + store review (typically 2–6 weeks)

### 4.2 Submission steps
1. **Product setup:** create product "NV-2.0", set genre (Sandbox,
   Simulation), rating (PEGI 3 / ESRB E — no blood, no gore), age gate.
2. **Store presence:** title, description, capsule images (EPIC sizes:
   3840×2160 key art, 1024×1024 portrait, 2560×1440 landscape, 800×450
   details), screenshots, videos, tags.
3. **Build configuration:** EGS requires a build config + a manifest.
   - Linux: point at `nv2_engine` (chmod +x) + `nv2_launcher.ebin`.
   - Windows: `nv2_engine.exe` + launcher.ebin.
   - First executable = `nv2_launcher.ebin` so the DRM/license gate runs
     before the game; pass `--key <key>` from the EGS entitlements flow
     if keyless delivery is desired (or embed the entitlement check).
4. **SDK/entitlements:** for paid keys EGS can issue your `keygen` output
   (section 2) as keys; for keyless, integrate Epic Online Services
   entitlement check into the launcher (next iteration).
5. **Review checklist:** crash-free on target, no debug builds, EULA,
   privacy policy, support email, refund policy.
6. **Release:** set price (section 5), submit for certification.

### 4.3 EGS page copy (EN / PL)
Same short copy as itch.io (section 3.3) + long description with features:
real NASA POWER climate per world, AI-driven terrain, day/night survival,
3×3 crafting, hostiles, achievements, save/load, offline fallback.

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
| Epic Games Store | $9.99 | 12% | keyless (EOS) or key-based |

### Regional notes
- PL storefront: 42 PLN flat on itch.io; EGS regionalizes automatically.
- Windows build must exist before EGS submission — run `--target windows`
  on a rustup machine (documented in section 2).

---

## 6. Go-live checklist
- [ ] Screenshots & capsule art generated
- [ ] itch.io page live (Linux build + price)
- [ ] Windows build produced (`--target windows`) and pushed to both stores
- [ ] 50 launch keys generated for reviewers/influencers
- [ ] EGS account verification started (tax/payout)
- [ ] EGS submission + certification
- [ ] Release announcement (post on itch + socials)

**Security reminder:** never commit `github-token.md` or the DRM secret.
The secret lives only on the build machine (and in your password manager).
