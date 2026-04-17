// Decoration System
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecorationType { Bush, Flower, Grass, Fern }

#[derive(Clone, Debug)]
pub struct DecorationInstance {
    pub x: f32, pub y: f32, pub z: f32,
    pub rotation: f32, pub scale: f32,
    pub decoration_type: DecorationType,
}

impl DecorationInstance {
    pub fn new(x: f32, y: f32, z: f32, dt: DecorationType) -> Self {
        Self { x, y, z, rotation: 0.0, scale: 1.0, decoration_type: dt }
    }
    pub fn randomize(&mut self, seed: u64) {
        let mut rng = seed;
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        self.rotation = ((rng % 1000) as f32 / 1000.0) * 6.28318;
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        self.scale = 0.8 + ((rng % 400) as f32 / 1000.0);
    }
}

#[derive(Clone, Debug, Default)]
pub struct DecorationChunk { pub decorations: Vec<DecorationInstance> }

impl DecorationChunk {
    pub fn new() -> Self { Default::default() }
    pub fn add(&mut self, i: DecorationInstance) { self.decorations.push(i); }
}

pub struct DecorationManager {
    chunks: HashMap<(i32, i32), DecorationChunk>,
    total: usize,
}

impl DecorationManager {
    pub fn new() -> Self { Self { chunks: HashMap::new(), total: 0 } }
    pub fn add(&mut self, x: f32, y: f32, z: f32, dt: DecorationType) {
        let cx = (x / 16.0).floor() as i32;
        let cz = (z / 16.0).floor() as i32;
        let mut inst = DecorationInstance::new(x, y, z, dt);
        inst.randomize((cx as u64) * 73856093 ^ (cz as u64) * 19349663);
        self.chunks.entry((cx, cz)).or_insert_with(Default::default).add(inst);
        self.total += 1;
    }
    pub fn get(&self, cx: i32, cz: i32) -> Option<&DecorationChunk> { self.chunks.get(&(cx, cz)) }
    pub fn total(&self) -> usize { self.total }
}
