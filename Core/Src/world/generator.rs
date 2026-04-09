use super::chunk::{Chunk, GeneratedChunk};
use super::biomes::BiomeGenerator;
use crate::settings::SharedSettings;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver};
use rayon::prelude::*;

/// Message sent from generator threads to the main thread.
pub enum GeneratorMessage {
    ChunkReady(i32, i32, GeneratedChunk),
}

/// Parallel chunk generator backed by rayon.
///
/// Chunks are dispatched onto the global rayon thread-pool in batches;
/// results come back through an mpsc channel that the main thread drains
/// each frame.  The work-queue is bounded to prevent memory growth during
/// fast camera movement.
pub struct ChunkGenerator {
    tx:             mpsc::Sender<GeneratorMessage>,
    work_queue:     Arc<Mutex<Vec<(i32, i32)>>>,
    in_flight:      Arc<Mutex<std::collections::HashSet<(i32, i32)>>>,
    max_queue_size: usize,
    gen:            Arc<BiomeGenerator>,
}

impl ChunkGenerator {
    /// Hard cap: chunks queued but not yet dispatched.
    const MAX_QUEUE_SIZE: usize = 128;
    /// Maximum chunks dispatched to rayon per flush call.
    const BATCH_SIZE: usize = 8;

    pub fn new_with_seed(seed: u32) -> (Self, Receiver<GeneratorMessage>) {
        Self::new_with_seed_and_settings(seed, SharedSettings::default())
    }

    pub fn new_with_seed_and_settings(seed: u32, settings: SharedSettings) -> (Self, Receiver<GeneratorMessage>) {
        let (tx, rx) = mpsc::channel();
        let gen      = Arc::new(BiomeGenerator::new_with_settings(seed, settings));
        (
            Self {
                tx,
                work_queue:     Arc::new(Mutex::new(Vec::with_capacity(Self::MAX_QUEUE_SIZE))),
                in_flight:      Arc::new(Mutex::new(std::collections::HashSet::new())),
                max_queue_size: Self::MAX_QUEUE_SIZE,
                gen,
            },
            rx,
        )
    }

    pub fn new() -> (Self, Receiver<GeneratorMessage>) {
        Self::new_with_seed(42)
    }

    /// Add a chunk coordinate to the work-queue (deduplicated, bounded).
    pub fn queue_chunk(&self, cx: i32, cz: i32) {
        let mut queue = match self.work_queue.try_lock() {
            Ok(q) => q,
            Err(_) => return,
        };
        if queue.len() >= self.max_queue_size { return; }
        let in_flight = self.in_flight.try_lock();
        let already_flying = in_flight.map(|s| s.contains(&(cx, cz))).unwrap_or(false);
        if !already_flying && !queue.iter().any(|&(x, z)| x == cx && z == cz) {
            queue.push((cx, cz));
        }
    }

    /// Add many coordinates at once (deduplication included).
    pub fn queue_chunks(&self, coords: &[(i32, i32)]) {
        let mut queue = match self.work_queue.try_lock() {
            Ok(q) => q,
            Err(_) => return,
        };
        let in_flight = self.in_flight.try_lock();
        for &(cx, cz) in coords {
            if queue.len() >= self.max_queue_size { break; }
            let flying = in_flight.as_ref().map(|s| s.contains(&(cx, cz))).unwrap_or(false);
            if !flying && !queue.iter().any(|&(x, z)| x == cx && z == cz) {
                queue.push((cx, cz));
            }
        }
    }

    /// Dispatch up to `BATCH_SIZE` chunks from the queue onto rayon.
    /// Call this once per frame from the main thread.
    pub fn flush(&self) {
        // Drain up to BATCH_SIZE items from the front of the queue
        let batch: Vec<(i32, i32)> = {
            let mut queue = match self.work_queue.try_lock() {
                Ok(q) => q,
                Err(_) => return,
            };
            let n = queue.len().min(Self::BATCH_SIZE);
            if n == 0 { return; }
            // Take from the front (closest-first ordering maintained by caller)
            queue.drain(..n).collect()
        };

        // Mark batch as in-flight
        if let Ok(mut s) = self.in_flight.try_lock() {
            for &coord in &batch { s.insert(coord); }
        }

        let tx      = self.tx.clone();
        let gen     = self.gen.clone();
        let in_fl   = self.in_flight.clone();

        // Spawn onto rayon — each item gets its own parallel job
        rayon::spawn(move || {
            batch.into_par_iter().for_each(|(cx, cz)| {
                let chunk = Chunk::generate(cx, cz, &gen);
                let _ = tx.send(GeneratorMessage::ChunkReady(cx, cz, chunk));
                if let Ok(mut s) = in_fl.try_lock() { s.remove(&(cx, cz)); }
            });
        });
    }

    /// Current pending work depth (queue + in-flight).
    pub fn queue_depth(&self) -> usize {
        let q = self.work_queue.try_lock().map(|q| q.len()).unwrap_or(0);
        let f = self.in_flight.try_lock().map(|s| s.len()).unwrap_or(0);
        q + f
    }

    pub fn is_overloaded(&self) -> bool {
        self.queue_depth() > self.max_queue_size / 2
    }

    pub fn generator(&self) -> &Arc<BiomeGenerator> { &self.gen }
}

impl Default for ChunkGenerator {
    fn default() -> Self {
        let (gen, _rx) = Self::new();
        gen
    }
}
