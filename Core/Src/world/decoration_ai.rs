// Decoration AI
use super::ai_generator::AISystem;
use super::biomes::BiomeGenerator;
use super::decorations::DecorationType;

pub struct DecorationAI;
impl DecorationAI {
    pub fn populate(
        deco_mgr: &mut super::decorations::DecorationManager,
        gen: &BiomeGenerator,
        ai: &AISystem,
        cx: i32,
        cz: i32,
    ) {
        for gy in 0..4 {
            for gx in 0..4 {
                let lx = (gx * 4) as f32 + 2.0;
                let lz = (gy * 4) as f32 + 2.0;
                let wx = cx * 16 + lx as i32;
                let wz = cz * 16 + lz as i32;

                let sample = gen.sample_column(wx, wz);
                if sample.water_top > sample.surface {
                    continue;
                }

                let features = [
                    (sample.surface as f32 / 256.0).min(1.0),
                    (sample.landness * 0.5) as f32,
                    sample.temperature as f32,
                    sample.humidity as f32,
                    0.5,
                    0.5,
                    0.7,
                    0.5,
                ];

                let (_block, conf) = ai.predict_vegetation(&features);
                if conf < 0.4 {
                    continue;
                }

                // NV2.0 MeMLP: the modular biome head picks the decoration
                // style from the climate features (0..9, BiomeId order).
                let biome_idx = ai.predict_biome(&features);
                let deco = match biome_idx {
                    5 | 6 => DecorationType::Fern,   // Swamp / Taiga
                    4 | 3 => DecorationType::Flower, // DarkForest / Forest
                    7 => DecorationType::Bush,       // Desert — sparse
                    _ => {
                        if sample.humidity > 0.6 {
                            DecorationType::Fern
                        } else {
                            DecorationType::Bush
                        }
                    }
                };

                let y = (sample.surface + 1) as f32;
                deco_mgr.add(lx, y, lz, deco);
            }
        }
    }
}
