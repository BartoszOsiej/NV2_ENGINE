use std::collections::HashMap;
use std::path::Path;
use std::fs;
use log::{info, warn};

/// Registry for automatically loading and mapping block textures
/// 
/// **Design**: This system scans the Assets/Blocks/ directory and dynamically
/// builds a mapping from block names to their texture file paths. This enables:
/// - Zero-configuration texture loading
/// - Easy mod/extension system (drop textures in Assets/Blocks/)
/// - No need to manually register every texture
///
/// **File Naming Convention**: Textures should be named as:
/// - `blockname.png` - main texture used for all faces
/// - `blockname_top.png` - override for top face
/// - `blockname_bottom.png` - override for bottom face
/// - `blockname_side.png` - override for side faces
#[derive(Debug, Clone)]
pub struct TextureRegistry {
    /// Mapping of block names to their texture paths
    textures: HashMap<String, BlockTexture>,
    /// Base path to the textures directory
    base_path: String,
}

#[derive(Debug, Clone)]
pub struct BlockTexture {
    /// Main texture file (used if specific faces not found)
    pub main: String,
    /// Top face override (if exists)
    pub top: Option<String>,
    /// Bottom face override (if exists)
    pub bottom: Option<String>,
    /// Side face override (if exists)
    pub side: Option<String>,
}

impl TextureRegistry {
    /// Create a new texture registry by scanning the given directory
    /// 
    /// **Parameters**:
    /// - `texture_dir`: Path to directory containing block textures (typically "Assets/Blocks/")
    ///
    /// **Returns**: TextureRegistry with all discovered textures
    pub fn new(texture_dir: &str) -> Self {
        let mut textures = HashMap::new();
        
        // Attempt to read directory
        match fs::read_dir(texture_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if !metadata.is_file() {
                            continue; // Skip directories
                        }
                    }
                    
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "png") {
                        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                            // Extract block name (remove .png extension)
                            let block_name = filename.strip_suffix(".png").unwrap_or(filename);
                            
                            // Extract base block name (remove _top, _bottom, _side suffixes)
                            let (base_name, suffix) = extract_base_name(block_name);
                            
                            // Get or create texture entry
                            let texture = textures.entry(base_name.to_string())
                                .or_insert_with(|| BlockTexture {
                                    main: format!("{}/{}.png", texture_dir, base_name),
                                    top: None,
                                    bottom: None,
                                    side: None,
                                });
                            
                            // Organize texture variants
                            match suffix {
                                Some("top") => texture.top = Some(format!("{}/{}", texture_dir, filename)),
                                Some("bottom") => texture.bottom = Some(format!("{}/{}", texture_dir, filename)),
                                Some("side") => texture.side = Some(format!("{}/{}", texture_dir, filename)),
                                _ => {} // Main texture already set
                            }
                        }
                    }
                }
                
                info!("Loaded {} unique block textures from directory: {}", textures.len(), texture_dir);
            }
            Err(e) => {
                warn!("Failed to read texture directory {}: {}", texture_dir, e);
            }
        }
        
        Self {
            textures,
            base_path: texture_dir.to_string(),
        }
    }
    
    /// Get texture for a specific block and face
    /// 
    /// **Parameters**:
    /// - `block_name`: Name of the block (e.g., "oak_log", "stone", "dirt")
    /// - `face`: Which face ("top", "bottom", "side")
    ///
    /// **Returns**: Path to texture file, or None if block/face not found
    pub fn get_texture(&self, block_name: &str, face: &str) -> Option<String> {
        self.textures.get(block_name).and_then(|texture| {
            match face {
                "top" => texture.top.clone().or_else(|| Some(texture.main.clone())),
                "bottom" => texture.bottom.clone().or_else(|| Some(texture.main.clone())),
                "side" => texture.side.clone().or_else(|| Some(texture.main.clone())),
                _ => Some(texture.main.clone()),
            }
        })
    }
    
    /// Get all available block names
    pub fn block_names(&self) -> Vec<&str> {
        self.textures.keys().map(|s| s.as_str()).collect()
    }
    
    /// Check if a specific block texture exists
    pub fn has_block(&self, block_name: &str) -> bool {
        self.textures.contains_key(block_name)
    }
    
    /// Get number of registered blocks
    pub fn block_count(&self) -> usize {
        self.textures.len()
    }
    
    /// Get the base path this registry was initialized with
    pub fn base_path(&self) -> &str {
        &self.base_path
    }
}

/// Split a texture filename into base block name and face variant
/// 
/// Examples:
/// - "oak_log" -> ("oak_log", None)
/// - "oak_log_top" -> ("oak_log", Some("top"))
/// - "stone_bricks_side" -> ("stone_bricks", Some("side"))
fn extract_base_name(filename: &str) -> (&str, Option<&str>) {
    // Check for common face suffixes
    if let Some(pos) = filename.rfind("_top") {
        if pos + 4 == filename.len() {
            return (&filename[..pos], Some("top"));
        }
    }
    if let Some(pos) = filename.rfind("_bottom") {
        if pos + 7 == filename.len() {
            return (&filename[..pos], Some("bottom"));
        }
    }
    if let Some(pos) = filename.rfind("_side") {
        if pos + 5 == filename.len() {
            return (&filename[..pos], Some("side"));
        }
    }
    
    (filename, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_base_name() {
        assert_eq!(extract_base_name("oak_log"), ("oak_log", None));
        assert_eq!(extract_base_name("oak_log_top"), ("oak_log", Some("top")));
        assert_eq!(extract_base_name("stone_bricks_side"), ("stone_bricks", Some("side")));
        assert_eq!(extract_base_name("stone_bricks_bottom"), ("stone_bricks", Some("bottom")));
    }
}