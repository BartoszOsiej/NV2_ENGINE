# 🚀 AI System - Phase 2 Extension Guide

## Roadmap: From Local Learning to Internet-Connected AI

### Current Status (Phase 1) ✅
- ✅ Local-only AI training
- ✅ Real-time vegetation placement
- ✅ No network dependency
- ✅ 1.2 KB model size

### Phase 2 Goals (Planned)

## Feature 1: Internet-Based Dataset Integration

### Goal
Download professional terrain data from cloud to train the AI faster

### Implementation Steps

#### Step 1: Add HTTP Client to Cargo.toml
```toml
[dependencies]
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
```

#### Step 2: Create Dataset Download Module
```rust
// ai_generator.rs - add new module

#[derive(serde::Deserialize)]
pub struct TerrainDataset {
    pub samples: Vec<[f32; 8]>,
    pub targets: Vec<[f32; 4]>,
}

pub async fn download_training_data(biome: &str) -> Result<TerrainDataset> {
    let url = format!("https://api.nvengine.io/datasets/{}", biome);

    let response = reqwest::get(&url)
        .await?
        .json::<TerrainDataset>()
        .await?;

    Ok(response)
}
```

#### Step 3: Integrate with Training Loop
```rust
fn background_training_loop(ai: Arc<Mutex<TerrainAI>>, tx: Sender<AIMessage>) {
    let mut epoch = 0;

    // Phase 1: Download real data (if available)
    let real_data = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            download_training_data("forest").await.ok()
        });

    loop {
        epoch += 1;
        let mut total_loss = 0.0;

        // Use real data if available, else synthetic
        let data = if let Some(ref dataset) = real_data {
            dataset
        } else {
            // Fallback to synthetic
            generate_synthetic_dataset(100)
        };

        for (features, target) in data.samples.iter().zip(data.targets.iter()) {
            if let Ok(mut ai_lock) = ai.lock() {
                let loss = ai_lock.backward(features, target);
                total_loss += loss;
            }
        }

        // ... rest of loop ...
    }
}
```

### Data Format Specification

**API Endpoint**: `GET /api/v1/datasets/{biome}`
```json
{
  "biome": "forest",
  "version": "1.0",
  "timestamp": "2024-01-15T10:30:00Z",
  "samples": [
    {
      "height": 0.45,
      "slope": 0.23,
      "temperature": 0.68,
      "humidity": 0.72,
      "water_distance": 0.15,
      "vegetation_count": 0.55,
      "light_level": 0.82,
      "noise_seed": 0.34
    }
  ],
  "targets": [
    [0.0, 0.8, 0.15, 0.05],  // Fern (wet, shaded)
    [0.7, 0.1, 0.1, 0.1]     // Flower (bright, humid)
  ]
}
```

---

## Feature 2: Real-Time Texture Generation with AI

### Goal
Generate block textures procedurally using AI guidance, adapting to biome

### Implementation

#### Step 1: Create Texture Generator
```rust
// Add to ai_generator.rs

pub struct TextureGenerator {
    ai: Arc<Mutex<TerrainAI>>,
    cache: Arc<Mutex<HashMap<u64, Vec<u8>>>>,
}

impl TextureGenerator {
    pub fn new(ai: Arc<Mutex<TerrainAI>>) -> Self {
        Self {
            ai,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn generate(&self, seed: u64, block_type: BlockType, biome: BiomeId) -> Vec<u8> {
        // Check cache first
        if let Ok(cache) = self.cache.lock() {
            if let Some(texture) = cache.get(&seed) {
                return texture.clone();
            }
        }

        // Generate new texture
        let mut texture = self.generate_procedural(seed, block_type, biome);

        // Cache result
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(seed, texture.clone());
        }

        texture
    }

    fn generate_procedural(&self, seed: u64, block_type: BlockType, biome: BiomeId) -> Vec<u8> {
        let width = 16;
        let height = 16;
        let mut texture = vec![0u8; (width * height * 4) as usize];

        // AI-guided noise generation
        let mut rng = seed;
        for y in 0..height {
            for x in 0..width {
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);

                // Sample noise
                let base_noise = ((rng >> 16) & 0xFF) as f32 / 255.0;

                // Get biome hints from AI
                let biome_features = self.get_biome_features(biome);

                // Apply AI-guided color mapping
                let (r, g, b) = self.apply_ai_coloring(
                    block_type,
                    biome_features,
                    base_noise,
                    x as f32 / width as f32,
                    y as f32 / height as f32,
                );

                let idx = (y * width + x) as usize * 4;
                texture[idx] = r;
                texture[idx + 1] = g;
                texture[idx + 2] = b;
                texture[idx + 3] = 255;
            }
        }

        texture
    }

    fn apply_ai_coloring(
        &self,
        block: BlockType,
        biome_features: [f32; 4],
        noise: f32,
        x: f32,
        y: f32,
    ) -> (u8, u8, u8) {
        // Use AI to predict color
        let features = [
            noise,
            biome_features[0],  // temperature
            biome_features[1],  // humidity
            biome_features[2],  // altitude_factor
            biome_features[3],  // season_factor
            x,
            y,
            (x * x + y * y).sqrt(),  // distance from center
        ];

        let probs = if let Ok(ai) = self.ai.lock() {
            ai.forward(&features)
        } else {
            [0.25, 0.25, 0.25, 0.25]
        };

        // Map probabilities to color channels
        let r = (probs[0] * 255.0) as u8;
        let g = (probs[1] * 255.0) as u8;
        let b = (probs[2] * 255.0) as u8;

        (r, g, b)
    }

    fn get_biome_features(&self, biome: BiomeId) -> [f32; 4] {
        match biome {
            BiomeId::Forest => [0.6, 0.7, 0.5, 0.3],      // warm, humid, mid-altitude
            BiomeId::Desert => [0.9, 0.1, 0.4, 0.2],      // hot, dry, low
            BiomeId::Taiga => [0.2, 0.6, 0.6, 0.8],       // cold, humid, high
            BiomeId::Mountains => [0.4, 0.5, 0.8, 0.5],   // mild, variable, high
            BiomeId::Plains => [0.7, 0.4, 0.3, 0.4],      // warm, dry, flat
            BiomeId::Swamp => [0.5, 0.9, 0.2, 0.6],       // cool, very humid, low
            _ => [0.5, 0.5, 0.5, 0.5],
        }
    }
}
```

#### Step 2: Integrate with Renderer
```rust
// In renderer/texture_atlas.rs

pub struct TextureAtlas {
    // ... existing fields ...
    ai_textures: TextureGenerator,
}

impl TextureAtlas {
    pub fn get_texture_for_block(
        &mut self,
        block: BlockType,
        biome: BiomeId,
        seed: u64,
    ) -> TextureUV {
        // Check if this is an AI-generated texture
        if block.should_ai_generate() {
            let texture_data = self.ai_textures.generate(seed, block, biome);
            self.upload_to_gpu(&texture_data)
        } else {
            self.get_static_texture(block)
        }
    }
}
```

---

## Feature 3: Online Learning from Player Actions

### Goal
Learn player preferences and improve terrain suggestions

### Implementation

#### Step 1: Track Player Interactions
```rust
pub struct PlayerFeedback {
    pub features: [f32; 8],
    pub block_placed: BlockType,
    pub timestamp: u64,
    pub biome: BiomeId,
}

pub struct FeedbackCollector {
    feedback_buffer: Arc<Mutex<Vec<PlayerFeedback>>>,
}

impl FeedbackCollector {
    pub fn record_placement(
        &self,
        features: [f32; 8],
        block: BlockType,
        biome: BiomeId,
    ) {
        let feedback = PlayerFeedback {
            features,
            block_placed: block,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            biome,
        };

        if let Ok(mut buffer) = self.feedback_buffer.lock() {
            buffer.push(feedback);
        }
    }

    pub fn get_batch(&self, size: usize) -> Vec<PlayerFeedback> {
        if let Ok(mut buffer) = self.feedback_buffer.lock() {
            buffer.drain(..buffer.len().min(size)).collect()
        } else {
            Vec::new()
        }
    }
}
```

#### Step 2: Integrate Feedback into Training
```rust
fn background_training_loop(
    ai: Arc<Mutex<TerrainAI>>,
    tx: Sender<AIMessage>,
    feedback: Arc<FeedbackCollector>,
) {
    loop {
        // Get player feedback (priority 1)
        let player_feedback = feedback.get_batch(50);

        if !player_feedback.is_empty() {
            // Train on player data (they know what looks good!)
            let mut total_loss = 0.0;
            for pb in player_feedback {
                let target = block_to_target_vector(pb.block_placed);
                if let Ok(mut ai_lock) = ai.lock() {
                    let loss = ai_lock.backward(&pb.features, target);
                    total_loss += loss;
                }
            }
            let _ = tx.send(AIMessage::TrainingProgress {
                epoch,
                loss: total_loss / player_feedback.len() as f32,
            });
        } else {
            // Fall back to synthetic training
            // ... existing synthetic training code ...
        }
    }
}
```

---

## Feature 4: Cloud Model Sharing (Multiplayer)

### Goal
Share learned terrain styles between servers/players

### Implementation

#### Step 1: Model Serialization
```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct TerrainAICheckpoint {
    pub version: String,
    pub timestamp: u64,
    pub biome: String,
    pub w1: Vec<f32>,  // Flattened matrix
    pub b1: Vec<f32>,
    pub w2: Vec<f32>,
    pub b2: Vec<f32>,
    pub total_epochs: u32,
    pub loss_history: Vec<f32>,
}

impl TerrainAI {
    pub fn to_checkpoint(&self, biome: &str) -> TerrainAICheckpoint {
        TerrainAICheckpoint {
            version: "1.0".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            biome: biome.to_string(),
            w1: self.w1.to_shape((8 * 16,)).unwrap().to_vec(),
            b1: self.b1.to_vec(),
            w2: self.w2.to_shape((16 * 4,)).unwrap().to_vec(),
            b2: self.b2.to_vec(),
            total_epochs: self.training_samples as u32,
            loss_history: Vec::new(),  // Track training progress
        }
    }
}
```

#### Step 2: Upload Model to Cloud
```rust
pub async fn upload_model(checkpoint: TerrainAICheckpoint) -> Result<()> {
    let client = reqwest::Client::new();
    let url = "https://api.nvengine.io/models/upload";

    client.post(url)
        .json(&checkpoint)
        .send()
        .await?
        .error_for_status()?;

    println!("✓ Model uploaded for biome: {}", checkpoint.biome);
    Ok(())
}
```

#### Step 3: Download Community Models
```rust
pub async fn download_community_model(biome: &str) -> Result<TerrainAICheckpoint> {
    let url = format!("https://api.nvengine.io/models/best/{}", biome);

    let checkpoint = reqwest::get(&url)
        .await?
        .json::<TerrainAICheckpoint>()
        .await?;

    println!("✓ Downloaded community model for {}", biome);
    Ok(checkpoint)
}
```

---

## Feature 5: Multi-Scale Feature Learning

### Goal
Learn local AND regional terrain patterns

### Architecture Enhancement

```rust
pub struct MultiScaleAI {
    // Local scale: 3x3 blocks
    local_ai: TerrainAI,

    // Regional scale: 16x16 chunks
    regional_ai: TerrainAI,  // Same architecture, different training data

    // Global scale: 256x256 blocks (optional)
    global_ai: TerrainAI,
}

impl MultiScaleAI {
    pub fn predict_vegetation(
        &self,
        features: &[f32; 8],
        local_context: &LocalContext,
        regional_context: &RegionalContext,
    ) -> (BlockType, f32) {
        // Get predictions from all scales
        let local_pred = self.local_ai.forward(features);
        let regional_pred = self.regional_ai.forward(&regional_context.features);

        // Combine predictions (weighted average or voting)
        let combined = self.combine_predictions(&local_pred, &regional_pred);

        // Pick best overall choice
        let (idx, confidence) = combined.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((2, 0.0));

        (self.idx_to_block(idx), confidence)
    }
}
```

---

## Feature 6: Real-Time Terrain Editing with AI

### Goal
As player modifies terrain, AI helps maintain consistency

### Implementation

```rust
pub struct AITerrainEditor {
    ai: Arc<Mutex<TerrainAI>>,
    suggestion_cache: Arc<Mutex<HashMap<(i32, i32, i32), BlockType>>>,
}

impl AITerrainEditor {
    pub fn suggest_block_replacement(
        &self,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        removed_block: BlockType,
        nearby_blocks: &[BlockType],
    ) -> Option<BlockType> {
        // Extract features from neighborhood
        let features = self.extract_features_from_neighborhood(nearby_blocks);

        // Get AI suggestion
        let (suggested_block, confidence) = self.ai_system.predict_vegetation(&features);

        // Only suggest if high confidence
        if confidence > 0.7 {
            return Some(suggested_block);
        }

        None
    }
}
```

---

## Implementation Priority

### Phase 2 Timeline

**Week 1-2**: Internet Integration
- [ ] Implement `download_training_data()`
- [ ] Set up API endpoints
- [ ] Test with mock server

**Week 3-4**: Texture Generation
- [ ] Build `TextureGenerator`
- [ ] Integrate with rendering pipeline
- [ ] Create shader helpers for AI texture

**Week 5-6**: Player Learning
- [ ] Implement `FeedbackCollector`
- [ ] Wire into block placement system
- [ ] Test feedback training

**Week 7-8**: Cloud Sharing
- [ ] Model serialization
- [ ] Upload/download pipeline
- [ ] Community model selection UI

**Week 9-10**: Multi-Scale Learning
- [ ] Design regional AI
- [ ] Implement prediction combining
- [ ] Optimize performance

---

## Testing Phase 2 Features

### Unit Tests to Add
```rust
#[test]
fn test_dataset_download() {
    let data = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(download_training_data("forest"));
    assert!(data.is_ok());
}

#[test]
fn test_texture_generation() {
    let gen = TextureGenerator::new(Arc::new(Mutex::new(TerrainAI::new())));
    let tex = gen.generate(42, BlockType::Rose, BiomeId::Forest);
    assert_eq!(tex.len(), 16 * 16 * 4);  // 16x16 RGBA
}

#[test]
fn test_model_serialization() {
    let ai = TerrainAI::new();
    let checkpoint = ai.to_checkpoint("forest");
    assert_eq!(checkpoint.version, "1.0");
    assert_eq!(checkpoint.biome, "forest");
}
```

---

## Performance Targets (Phase 2)

| Feature | Target |
|---------|--------|
| Dataset download | < 1 second |
| Texture generation | < 5ms per block |
| Model upload | < 100ms |
| Community model sync | < 500ms |
| Multi-scale prediction | < 0.05ms |

---

**Ready to extend!** 🚀

For questions about implementation, refer to `AI_TECHNICAL_DOCS.md`
