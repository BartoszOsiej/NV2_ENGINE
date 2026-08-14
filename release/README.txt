====================================================
  NV-2.0 — Externum Edition
  Release build for Linux x86_64
====================================================

WHAT'S IN THIS PACKAGE
  nv2_engine            the game engine (Rust, wgpu)
  nv2_launcher.ebin     the DRM-protected launcher (Externum .ext build)
  nv2.key               a valid license key for this exact build

RUNNING THE GAME
  python3 nv2_launcher.ebin --key <license-key>

  The launcher verifies the license key and the integrity of the game
  binary, then starts NV-2.0. If the binary is modified or the key is
  wrong, the game refuses to start.

  Your personal license key is delivered separately (see the store page).
  Never share it — one key = one copy of the game.

FIRST RUN TIP
  The world is generated from a seed that maps to a real location on
  Earth. If the machine is online, NV-2.0 pulls the real annual climate
  for that location from the NASA POWER API (temperature, precipitation,
  humidity, solar radiation, wind) and shapes biomes, weather and the sky
  accordingly. Offline, a deterministic synthetic climate is used instead
  — the world is still fully playable.

CONTROLS
  WASD        move        Space    jump
  Mouse       look        LMB      mine
  Esc         pause menu  E        inventory / crafting
  T           console     /help    list of commands

  Survive the night, keep your health and hunger up, craft tools, fight
  off the hostiles and unlock achievements.

NOTES
  - Requires a GPU with Vulkan/DX12/Metal support (wgpu).
  - The launcher itself is an Externum artifact protected by Externum
    DRM (watermark, license gate, tamper detection).
====================================================
