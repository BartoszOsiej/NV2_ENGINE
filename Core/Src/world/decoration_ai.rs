// Decoration AI
use super::decorations::DecorationType;
use super::ai_generator::AISystem;
use super::biomes::BiomeGenerator;

pub struct DecorationAI;
impl DecorationAI {
    pub fn populate(
        deco_mgr: &mut super::decorations::DecorationManager,
        gen: &BiomeGenerator,
        ai: &AISystem,
        cx: i32, cz: i32,
    ) {
        for gy in 0..4 {
            for gx in 0..4 {
                let lx = (gx * 4) as f32 + 2.0;
                let lz = (gy * 4) as f32 + 2.0;
                let wx = cx * 16 + lx as i32;
                let wz = cz * 16 + lz as i32;
                
                let sample = gen.sample_column(wx, wz);
                if sample.water_top > sample.surface { continue; }
                
                let features = [
                    (sample.surface as f32 / 256.0).min(1.0),
                    (sample.landness * 0.5) as f32,
                    sample.temperature as f32,
                    sample.humidity as f32,
                    0.5, 0.5, 0.7, 0.5,
                ];
                
                let (_block, conf) = ai.predict_vegetation(&features);
                if conf < 0.4 { continue; }
                
                let y = (sample.surface + 1) as f32;
                if sample.humidity > 0.6 {
                    deco_mgr.add(lx, y, lz, DecorationType::Fern);
                } else {
                    deco_mgr.add(lx, y, lz, DecorationType::Bush);
                }
            }
        }
    }
}
