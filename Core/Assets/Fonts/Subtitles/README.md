Place subtitle fonts here.

Instructions:
- Put `Doto-VariableFont_ROND,wght.ttf` in the project root (or anywhere).
- On next run, the engine will attempt to move the file into this folder.
- If you prefer, move the file here manually.

This font will be used by the in-game subtitle renderer.

Usage:
- Drop `Doto-VariableFont_ROND,wght.ttf` in the project root (or add it anywhere).
- On next run the engine will attempt to move it into `Assets/Fonts/Subtitles/`.
- From game code you can display subtitles by calling:

	```rust
	// where `state` is your `renderer::State` instance
	state.show_subtitle("This is a subtitle message.");
	```

Notes:
- The renderer will rasterize the TTF at runtime using `fontdue` and render
	subtitles as a textured UI quad. If the font is not found, no subtitle will render.
