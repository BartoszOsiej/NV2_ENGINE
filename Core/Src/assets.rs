use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;

/// Block model definition loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockModel {
    Simple {
        name: String,
        /// Face textures: [top, bottom, front, back, right, left]
        textures: [String; 6],
        opaque: bool,
        breakable: bool,
    },
    Minecraft {
        /// Optional parent model reference
        parent: Option<String>,
        /// Textures object with keys like "all", "top", "bottom", etc.
        textures: HashMap<String, String>,
        /// Optional name (derived from filename)
        #[serde(default)]
        name: Option<String>,
    },
}

/// Crafting recipe from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Recipe {
    Shaped {
        name: String,
        pattern: Vec<String>,
        key: HashMap<char, String>,
        result: RecipeResult,
    },
    Shapeless {
        name: String,
        ingredients: Vec<String>,
        result: RecipeResult,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeResult {
    pub item: String,
    #[serde(default)]
    pub count: u32,
}

/// Normalized block model with consistent texture array
#[derive(Debug, Clone)]
pub struct NormalizedBlockModel {
    pub name: String,
    pub textures: [String; 6], // [top, bottom, front, back, right, left]
    pub opaque: bool,
    pub breakable: bool,
}

impl BlockModel {
    /// Convert to normalized format with [top, bottom, front, back, right, left] textures
    pub fn normalize(&self) -> NormalizedBlockModel {
        match self {
            BlockModel::Simple { name, textures, opaque, breakable } => {
                NormalizedBlockModel {
                    name: name.clone(),
                    textures: textures.clone(),
                    opaque: *opaque,
                    breakable: *breakable,
                }
            }
            BlockModel::Minecraft { parent, textures, name } => {
                // Handle parent inheritance
                let resolved_textures = Self::resolve_textures(parent.as_deref(), textures);
                NormalizedBlockModel {
                    name: name.clone().unwrap_or_else(|| "unknown".to_string()),
                    textures: resolved_textures,
                    opaque: true,  // Default for Minecraft models
                    breakable: true, // Default for Minecraft models
                }
            }
        }
    }

    /// Resolve textures based on parent inheritance
    fn resolve_textures(parent: Option<&str>, textures: &HashMap<String, String>) -> [String; 6] {
        match parent {
            Some("block/cube_all") => {
                // All faces use the same texture
                if let Some(all_tex) = textures.get("all") {
                    [all_tex.clone(), all_tex.clone(), all_tex.clone(), all_tex.clone(), all_tex.clone(), all_tex.clone()]
                } else {
                    [
                        "unknown".to_string(),
                        "unknown".to_string(),
                        "unknown".to_string(),
                        "unknown".to_string(),
                        "unknown".to_string(),
                        "unknown".to_string(),
                    ]
                }
            }
            Some("block/cube") => {
                // Individual face textures
                [
                    textures.get("up").or_else(|| textures.get("top")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("down").or_else(|| textures.get("bottom")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("north").or_else(|| textures.get("front")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("south").or_else(|| textures.get("back")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("east").or_else(|| textures.get("right")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("west").or_else(|| textures.get("left")).unwrap_or(&"unknown".to_string()).clone(),
                ]
            }
            _ => {
                // No parent or unknown parent - try to map individual textures
                [
                    textures.get("up").or_else(|| textures.get("top")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("down").or_else(|| textures.get("bottom")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("north").or_else(|| textures.get("front")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("south").or_else(|| textures.get("back")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("east").or_else(|| textures.get("right")).unwrap_or(&"unknown".to_string()).clone(),
                    textures.get("west").or_else(|| textures.get("left")).unwrap_or(&"unknown".to_string()).clone(),
                ]
            }
        }
    }
}

// Global cache for loaded block models (thread-safe)
thread_local! {
    static BLOCK_MODEL_CACHE: Mutex<HashMap<String, NormalizedBlockModel>> = Mutex::new(HashMap::new());
}

/// Loads and manages block models
pub struct BlockModelLoader;

impl BlockModelLoader {
    /// Load all block models from a directory and cache them
    pub fn load_all<P: AsRef<Path>>(dir: P) -> Result<HashMap<String, NormalizedBlockModel>> {
        let mut models = HashMap::new();
        let dir = dir.as_ref();
        
        // Try multiple possible paths relative to the executable
        let possible_paths = vec![
            dir.to_path_buf(),
            Path::new("../Assets/Models/Block/").to_path_buf(),
            Path::new("../../Assets/Models/Block/").to_path_buf(),
            Path::new("../../../Assets/Models/Block/").to_path_buf(),
            Path::new("./Assets/Models/Block/").to_path_buf(),
        ];
        
        let mut found_path = None;
        for path in &possible_paths {
            if path.exists() {
                found_path = Some(path.clone());
                break;
            }
        }
        
        let dir = match found_path {
            Some(path) => path,
            None => {
                eprintln!("⚠️ Block models directory not found. Tried:");
                for path in &possible_paths {
                    eprintln!("  {:?}", path);
                }
                return Ok(models);
            }
        };
        
        eprintln!("✓ Found block models at: {:?}", dir);
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map_or(false, |ext| ext == "json") {
                let file_stem = path.file_stem()
                    .and_then(OsStr::to_str)
                    .unwrap_or("unknown")
                    .to_string();

                match Self::load_single(&path, &file_stem) {
                    Ok(mut normalized) => {
                        if normalized.name == "unknown" {
                            normalized.name = file_stem.clone();
                        }
                        eprintln!("✓ Loaded block model: {}", normalized.name);
                        models.insert(normalized.name.clone(), normalized);
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to load block model {:?}: {}", path, e);
                        let fallback = NormalizedBlockModel {
                            name: file_stem.clone(),
                            textures: std::array::from_fn(|_| "unknown".to_string()),
                            opaque: true,
                            breakable: true,
                        };
                        models.insert(file_stem.clone(), fallback);
                    }
                }
            }
        }
        
        // Populate global cache
        BLOCK_MODEL_CACHE.with(|cache| {
            if let Ok(mut cache_guard) = cache.lock() {
                for (name, model) in &models {
                    cache_guard.insert(name.clone(), model.clone());
                }
            }
        });
        
        eprintln!("✓ Loaded {} block models", models.len());
        Ok(models)
    }
    
    /// Load a single block model from JSON
    pub fn load_single<P: AsRef<Path>>(path: P, file_stem: &str) -> Result<NormalizedBlockModel> {
        let content = fs::read_to_string(path)?;
        let json_value: Value = serde_json::from_str(&content)?;

        if let Ok(raw_model) = serde_json::from_value::<BlockModel>(json_value.clone()) {
            let mut normalized = raw_model.normalize();
            if normalized.name == "unknown" {
                normalized.name = file_stem.to_string();
            }
            return Ok(normalized);
        }

        let textures = Self::extract_textures(&json_value);
        Ok(NormalizedBlockModel {
            name: file_stem.to_string(),
            textures,
            opaque: true,
            breakable: true,
        })
    }

    fn extract_textures(value: &Value) -> [String; 6] {
        if let Some(textures) = value.get("textures").and_then(|v| v.as_object()) {
            return Self::resolve_texture_map(textures);
        }

        if let Some(elements) = value.get("elements").and_then(|v| v.as_array()) {
            let mut found = Vec::new();
            for element in elements {
                if let Some(faces) = element.get("faces").and_then(|v| v.as_object()) {
                    for face in faces.values() {
                        if let Some(texture) = face.get("texture").and_then(|t| t.as_str()) {
                            found.push(texture.trim_start_matches('#').to_string());
                        }
                    }
                }
            }
            if let Some(first) = found.first() {
                return [first.clone(), first.clone(), first.clone(), first.clone(), first.clone(), first.clone()];
            }
        }

        std::array::from_fn(|_| "unknown".to_string())
    }

    fn resolve_texture_map(map: &serde_json::Map<String, Value>) -> [String; 6] {
        let all = Self::get_texture_text(map, &["all"]);
        if let Some(all_tex) = all {
            return [all_tex.clone(), all_tex.clone(), all_tex.clone(), all_tex.clone(), all_tex.clone(), all_tex.clone()];
        }

        let top = Self::get_texture_text(map, &["up", "top"]).unwrap_or_else(|| "unknown".to_string());
        let bottom = Self::get_texture_text(map, &["down", "bottom"]).unwrap_or_else(|| "unknown".to_string());
        let north = Self::get_texture_text(map, &["north", "front", "side"]).unwrap_or_else(|| "unknown".to_string());
        let south = Self::get_texture_text(map, &["south", "back", "side"]).unwrap_or_else(|| "unknown".to_string());
        let east = Self::get_texture_text(map, &["east", "right", "side"]).unwrap_or_else(|| "unknown".to_string());
        let west = Self::get_texture_text(map, &["west", "left", "side"]).unwrap_or_else(|| "unknown".to_string());

        [top, bottom, north, south, east, west]
    }

    fn get_texture_text(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
        for &key in keys {
            if let Some(value) = map.get(key).and_then(|v| v.as_str()) {
                return Some(value.trim_start_matches('#').to_string());
            }
        }
        None
    }

    /// Get a cached block model by name
    pub fn get_model(name: &str) -> Option<NormalizedBlockModel> {
        BLOCK_MODEL_CACHE.with(|cache| {
            if let Ok(cache_guard) = cache.lock() {
                cache_guard.get(name).cloned()
            } else {
                None
            }
        })
    }
}

/// Manages crafting recipes
pub struct RecipeManager;

impl RecipeManager {
    /// Load all recipes from a directory
    pub fn load_all<P: AsRef<Path>>(dir: P) -> Result<HashMap<String, Recipe>> {
        let mut recipes = HashMap::new();
        let dir = dir.as_ref();
        
        if !dir.exists() {
            return Ok(recipes);
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map_or(false, |ext| ext == "json") {
                match Self::load_single(&path) {
                    Ok(recipe) => {
                        let name = match &recipe {
                            Recipe::Shaped { name, .. } => name.clone(),
                            Recipe::Shapeless { name, .. } => name.clone(),
                        };
                        recipes.insert(name, recipe);
                    }
                    Err(e) => {
                        eprintln!("Failed to load recipe {:?}: {}", path, e);
                    }
                }
            }
        }
        
        Ok(recipes)
    }
    
    /// Load a single recipe from JSON
    pub fn load_single<P: AsRef<Path>>(path: P) -> Result<Recipe> {
        let content = fs::read_to_string(path)?;
        let recipe: Recipe = serde_json::from_str(&content)?;
        Ok(recipe)
    }
    
    /// Validate a shapeless recipe against provided items
    pub fn validate_shapeless(
        recipe: &Recipe,
        items: &[String],
    ) -> bool {
        match recipe {
            Recipe::Shapeless { ingredients, .. } => {
                if ingredients.len() != items.len() {
                    return false;
                }
                
                let mut required = ingredients.clone();
                for item in items {
                    if let Some(pos) = required.iter().position(|r| r == item) {
                        required.remove(pos);
                    } else {
                        return false;
                    }
                }
                required.is_empty()
            }
            _ => false,
        }
    }
    
    /// Validate a shaped recipe against a 3x3 grid
    pub fn validate_shaped(
        recipe: &Recipe,
        grid: &[[Option<String>; 3]; 3],
    ) -> bool {
        match recipe {
            Recipe::Shaped { pattern, key, .. } => {
                // Simple validation - could be enhanced for rotation/reflection
                Self::match_pattern(pattern, key, grid)
            }
            _ => false,
        }
    }
    
    fn match_pattern(
        pattern: &[String],
        key: &HashMap<char, String>,
        grid: &[[Option<String>; 3]; 3],
    ) -> bool {
        if pattern.len() != 3 {
            return false;
        }
        
        for (y, pattern_row) in pattern.iter().enumerate() {
            if pattern_row.len() != 3 {
                return false;
            }
            
            for (x, pattern_char) in pattern_row.chars().enumerate() {
                let required_item = key.get(&pattern_char);
                let grid_item = &grid[y][x];
                
                match (required_item, grid_item) {
                    (None, None) => {} // Empty matches empty
                    (Some(req), Some(grid_val)) => {
                        if req != grid_val {
                            return false;
                        }
                    }
                    _ => return false, // Mismatch
                }
            }
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_shapeless_recipe() {
        let recipe = Recipe::Shapeless {
            name: "test".into(),
            ingredients: vec!["wood".into(), "stick".into()],
            result: RecipeResult {
                item: "planks".into(),
                count: 2,
            },
        };
        
        assert!(RecipeManager::validate_shapeless(
            &recipe,
            &["stick".into(), "wood".into()]
        ));
        
        assert!(!RecipeManager::validate_shapeless(
            &recipe,
            &["wood".into()]
        ));
    }
}

/// Ensure subtitle font is placed into `Assets/Fonts/Subtitles/`.
///
/// If `Doto-VariableFont_ROND,wght.ttf` is found in common locations (project root
/// or parent folders) it will be moved (or copied) into the assets folder and the
/// destination path returned. Returns `Ok(None)` if not found.
pub fn ensure_subtitle_font() -> Result<Option<std::path::PathBuf>> {
    use std::path::PathBuf;

    let fname = "Doto-VariableFont_ROND,wght.ttf";
    let dest_dir = Path::new("Assets/Fonts/Subtitles");
    if !dest_dir.exists() {
        if let Err(e) = fs::create_dir_all(dest_dir) {
            eprintln!("Failed to create subtitle font directory {:?}: {}", dest_dir, e);
            return Ok(None);
        }
    }
    let dest = dest_dir.join(fname);
    if dest.exists() {
        return Ok(Some(dest));
    }

    let candidates: Vec<PathBuf> = vec![
        Path::new(fname).to_path_buf(),
        Path::new(&format!("./{}", fname)).to_path_buf(),
        Path::new(&format!("../{}", fname)).to_path_buf(),
        Path::new(&format!("../../{}", fname)).to_path_buf(),
        Path::new(&format!("./Assets/{}", fname)).to_path_buf(),
    ];

    for cand in candidates {
        if cand.exists() && cand.is_file() {
            // try to move; if rename fails, try copy+remove
            match fs::rename(&cand, &dest) {
                Ok(_) => {
                    eprintln!("Moved subtitle font to {:?}", dest);
                    return Ok(Some(dest));
                }
                Err(rename_err) => {
                    match fs::copy(&cand, &dest) {
                        Ok(_) => {
                            let _ = fs::remove_file(&cand);
                            eprintln!("Copied subtitle font to {:?}", dest);
                            return Ok(Some(dest));
                        }
                        Err(copy_err) => {
                            eprintln!("Failed to move/copy subtitle font {:?}: {}, {}", cand, rename_err, copy_err);
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}
