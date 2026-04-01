use super::chunk::Chunk;
use super::biomes::BiomeGenerator;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// Message sent from generator thread to main thread
pub enum GeneratorMessage {
    ChunkReady(i32, i32, Chunk),
}

/// Optimized async chunk generator with bounded work queue
/// 
/// Key Improvements:
/// - Hard limit on work queue (MAX_QUEUE_SIZE) to prevent unbounded memory growth
/// - Main thread never blocks (all ops are try_recv/try_lock)
/// - Background thread processes chunks one at a time
/// - Frame-time respecting: doesn't force chunk generation to finish immediately
pub struct ChunkGenerator {
    tx: mpsc::Sender<GeneratorMessage>,
    work_queue: Arc<Mutex<Vec<(i32, i32)>>>,
    pending: Arc<Mutex<Vec<(i32, i32)>>>,
    max_queue_size: usize,
    gen: Arc<BiomeGenerator>,
}

impl ChunkGenerator {
    /// Maximum chunks in queue at any time (prevents memory exhaustion)
    const MAX_QUEUE_SIZE: usize = 64;
    
    /// Create new generator with seed
    pub fn new_with_seed(seed: u32) -> (Self, Receiver<GeneratorMessage>) {
        let (tx, rx) = mpsc::channel();
        let work_queue = Arc::new(Mutex::new(Vec::with_capacity(Self::MAX_QUEUE_SIZE)));
        let pending = Arc::new(Mutex::new(Vec::new()));
        let gen = Arc::new(BiomeGenerator::new(seed));

        // Spawn worker thread that manages generation
        let tx_clone = tx.clone();
        let queue_clone = work_queue.clone();
        let pending_clone = pending.clone();
        let gen_clone = gen.clone();

        thread::spawn(move || {
            loop {
                // Pop one chunk from queue (non-blocking)
                let maybe_chunk = {
                    let mut queue = queue_clone.lock().unwrap();
                    queue.pop()
                };

                if let Some((cx, cz)) = maybe_chunk {
                    // Mark as pending
                    {
                        let mut pend = pending_clone.lock().unwrap();
                        pend.push((cx, cz));
                    }

                    // Generate on worker thread (off main thread)
                    let chunk = Chunk::generate(cx, cz, &gen_clone);
                    let _ = tx_clone.send(GeneratorMessage::ChunkReady(cx, cz, chunk));
                    
                    // Remove from pending
                    {
                        let mut pend = pending_clone.lock().unwrap();
                        pend.retain(|&(x, z)| !(x == cx && z == cz));
                    }
                } else {
                    // Queue empty, sleep briefly to reduce CPU usage
                    thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        });

        (
            Self {
                tx,
                work_queue,
                pending,
                max_queue_size: Self::MAX_QUEUE_SIZE,
                gen,
            },
            rx,
        )
    }

    pub fn new() -> (Self, Receiver<GeneratorMessage>) {
        Self::new_with_seed(42)
    }

    /// Queue single chunk (respects queue size limit)
    pub fn queue_chunk(&self, cx: i32, cz: i32) {
        let mut queue = match self.work_queue.try_lock() {
            Ok(q) => q,
            Err(_) => return, // Don't block if lock is held
        };

        // Enforce maximum queue size
        if queue.len() >= self.max_queue_size {
            return; // Queue full, drop request
        }

        // Avoid duplicates
        if !queue.iter().any(|&(x, z)| x == cx && z == cz) {
            queue.push((cx, cz));
        }
    }

    /// Queue multiple chunks (respects total queue size)
    pub fn queue_chunks(&self, coords: &[(i32, i32)]) {
        let mut queue = match self.work_queue.try_lock() {
            Ok(q) => q,
            Err(_) => return,
        };

        if queue.len() >= self.max_queue_size {
            return; // Queue full
        }

        let pending = match self.pending.try_lock() {
            Ok(p) => p,
            Err(_) => return,
        };

        let mut added = 0;
        for &(cx, cz) in coords {
            if queue.len() + added >= self.max_queue_size {
                break; // Stop if we'd exceed max
            }

            let queued = queue.iter().any(|&(x, z)| x == cx && z == cz);
            let pend = pending.iter().any(|&(x, z)| x == cx && z == cz);

            if !queued && !pend {
                queue.push((cx, cz));
                added += 1;
            }
        }
    }

    /// Get current queue depth (non-blocking)
    pub fn queue_depth(&self) -> usize {
        let q_len = self.work_queue.try_lock().map(|q| q.len()).unwrap_or(0);
        let p_len = self.pending.try_lock().map(|p| p.len()).unwrap_or(0);
        q_len + p_len
    }

    /// Check if generator is overloaded (queue > 50% capacity)
    pub fn is_overloaded(&self) -> bool {
        self.queue_depth() > self.max_queue_size / 2
    }
}

impl Default for ChunkGenerator {
    fn default() -> Self {
        let (gen, _rx) = Self::new();
        gen
    }
}
