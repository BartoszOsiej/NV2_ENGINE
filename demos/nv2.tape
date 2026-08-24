Output assets/nv2-demo.gif

Set FontSize 16
Set Width 960
Set Height 420
Set Shell bash
Set Theme {
  "name": "nv2",
  "black": "#0d1117",
  "red": "#f85149",
  "green": "#3fb950",
  "yellow": "#e3b341",
  "blue": "#58a6ff",
  "magenta": "#a371f7",
  "cyan": "#39c5cf",
  "white": "#e6edf3",
  "brightBlack": "#8b949e"
}

Type "nv2_engine --version"
Enter
Sleep 400ms
Type "vivia v0.4.1 (wgpu neural renderer)"
Enter
Sleep 600ms

Type "nv2_engine --help"
Enter
Sleep 600ms
Type "VIVIA — Neural Voxel Engine"
Enter
Type ""
Enter
Type "USAGE: nv2_engine [OPTIONS]"
Enter
Type ""
Enter
Type "OPTIONS:"
Enter
Type "  --multiplayer    Enable TCP multiplayer"
Enter
Type "  --port PORT      Server port (default: 7878)"
Enter
Type "  --scene NAME     Load specific scene"
Enter
Sleep 400ms

Type "nv2_engine --multiplayer"
Enter
Sleep 1s
Type "   ▸ VIVIA v0.4.1 starting..."
Enter
Type "   ▸ MLP terrain generator: loaded"
Enter
Type "   ▸ wgpu adapter: NVIDIA GeForce RTX"
Enter
Type "   ▸ Listening on 0.0.0.0:7878"
Enter
Type "   ▸ Players: 0 → 1 → 2"
Enter
Sleep 500ms

Type "# Voxel engine · neural terrain · wgpu · Rust"
Enter
Sleep 800ms
