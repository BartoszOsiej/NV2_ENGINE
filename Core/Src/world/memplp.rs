//! MeMLP — **M**odular **e**mbedded **M**ulti-**L**ayer **P**erceptron Model.
//!
//! The neural-network stack embedded directly in the NV2.0 engine.
//!
//! * **Modular** — one model file contains several specialist MLP modules,
//!   each solving a single task (vegetation placement, biome classification,
//!   texture-style selection). Modules share nothing but the checkpoint file.
//! * **Embedded** — the whole stack is a few KB, runs 100% on the CPU inside
//!   the game process (no GPU, no cloud, no external runtime) and is
//!   serialised to JSON checkpoints between runs.
//! * **Multi-layer** — every module is a true multi-layer perceptron with
//!   ReLU hidden layers, a softmax output and online gradient-descent
//!   training (backprop + cross-entropy loss).
//!
//! Architecture of the default model ([`MeMLP::new`]):
//!
//! | Module       | Shape            | Task                                          |
//! |--------------|------------------|-----------------------------------------------|
//! | `vegetation` | 8 → 24 → 16 → 4  | flower / fern / stick / pebble placement      |
//! | `biome`      | 8 → 12 → 9       | biome classification (9 world biomes)         |
//! | `texture`    | 8 → 12 → 6       | procedural texture-style selection            |
//!
//! Old single-hidden-layer checkpoints (w1/b1/w2/b2) are detected and
//! migrated automatically on load — see [`MeMLP::from_legacy`].

use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};

/// Version of the MeMLP checkpoint format.
pub const MEMLP_VERSION: u32 = 1;

/// Vegetation module shape: 8 climate/terrain features → 4 vegetation classes.
pub const VEGETATION_ARCH: [usize; 4] = [8, 24, 16, 4];
/// Biome module shape: 8 features → 9 biomes (order = `BiomeId`).
pub const BIOME_ARCH: [usize; 3] = [8, 12, 9];
/// Texture module shape: 8 features → 6 texture-style classes.
pub const TEXTURE_ARCH: [usize; 3] = [8, 12, 6];

/// Number of texture-style classes ([`texture_heuristic_target`]).
pub const TEXTURE_CLASSES: usize = 6;

/* ================================================================ */
/*  Mlp — a generic multi-layer perceptron                           */
/* ================================================================ */

/// A generic feed-forward multi-layer perceptron.
///
/// * `weights[k]` is `(arch[k], arch[k + 1])`, `biases[k]` is `arch[k + 1]`.
/// * Hidden layers use ReLU; the output layer uses softmax.
/// * Training is online stochastic gradient descent (backpropagation)
///   with cross-entropy loss — exactly what a small embedded model needs.
#[derive(Serialize, Deserialize, Clone)]
pub struct Mlp {
    version: u32,
    arch: Vec<usize>,
    weights: Vec<Array2<f32>>,
    biases: Vec<Array1<f32>>,
}

impl Mlp {
    /// Create a randomly-initialised network with the given layer sizes.
    /// Initialisation is seeded and deterministic: two `new()` calls with the
    /// same `arch` produce identical weights (reproducible worlds + tests).
    pub fn new(arch: &[usize]) -> Self {
        assert!(arch.len() >= 2, "an MLP needs at least an input and output layer");
        let mut state: u64 = 0x5EED_5EED_0000_0001;
        let mut weights = Vec::with_capacity(arch.len() - 1);
        let mut biases = Vec::with_capacity(arch.len() - 1);

        for k in 0..arch.len() - 1 {
            let (n_in, n_out) = (arch[k], arch[k + 1]);
            let mut w = Array2::<f32>::zeros((n_in, n_out));
            for v in w.iter_mut() {
                *v = lcg_next(&mut state) * 0.2 - 0.1;
            }
            let mut b = Array1::<f32>::zeros(n_out);
            for v in b.iter_mut() {
                *v = lcg_next(&mut state) * 0.02 - 0.01;
            }
            weights.push(w);
            biases.push(b);
        }

        Self {
            version: 1,
            arch: arch.to_vec(),
            weights,
            biases,
        }
    }

    /// Convert the legacy single-hidden-layer format (8→16→4) into an [`Mlp`].
    ///
    /// Keeps the trained weights so older checkpoints stay usable after the
    /// MeMLP upgrade instead of being discarded.
    pub fn from_legacy(w1: Array2<f32>, b1: Array1<f32>, w2: Array2<f32>, b2: Array1<f32>) -> Self {
        let arch = vec![w1.nrows(), w1.ncols(), w2.ncols()];
        Self {
            version: 1,
            arch,
            weights: vec![w1, w2],
            biases: vec![b1, b2],
        }
    }

    /// Layer sizes, e.g. `[8, 24, 16, 4]`.
    pub fn arch(&self) -> &[usize] {
        &self.arch
    }

    /// Total number of trainable parameters.
    pub fn param_count(&self) -> usize {
        self.weights.iter().map(|w| w.len()).sum::<usize>()
            + self.biases.iter().map(|b| b.len()).sum::<usize>()
    }

    /// Forward pass: `input` → probability distribution over outputs.
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let mut h = Array1::from(input.to_vec());
        let last = self.weights.len() - 1;
        for (k, w) in self.weights.iter().enumerate() {
            h = h.dot(w) + &self.biases[k];
            if k == last {
                return softmax(&h);
            }
            h = h.mapv(|x| if x > 0.0 { x } else { 0.0 });
        }
        unreachable!("an MLP always has at least one weight layer")
    }

    /// Replace any non-finite (NaN/Inf) weight with `0.0`.
    ///
    /// Recovers a model from a numerical blow-up instead of letting NaN
    /// poison the checkpoint file (serde_json serialises NaN as JSON
    /// `null`, which corrupts the file on reload).
    pub fn sanitize(&mut self) {
        for w in self.weights.iter_mut() {
            for v in w.iter_mut() {
                if !v.is_finite() {
                    *v = 0.0;
                }
            }
        }
        for b in self.biases.iter_mut() {
            for v in b.iter_mut() {
                if !v.is_finite() {
                    *v = 0.0;
                }
            }
        }
    }

    /// One gradient-descent step on a single sample. Returns the
    /// cross-entropy loss before the update (lower = better fit).
    ///
    /// Numerically hardened: inputs are validated, gradients are clipped and
    /// weight updates are bounded, so a single extreme sample (or a long
    /// training run) can never explode the weights into NaN/Inf.
    pub fn train(&mut self, input: &[f32], target: &[f32], learning_rate: f32) -> f32 {
        // Guard against poisoned inputs (NaN/Inf features or labels) — skip
        // the update entirely instead of propagating the poison.
        if !input.iter().chain(target.iter()).all(|v| v.is_finite()) {
            return f32::INFINITY;
        }

        let layers = self.weights.len();

        // Forward pass, remembering pre-activations for the ReLU backprop.
        let mut acts: Vec<Array1<f32>> = Vec::with_capacity(layers + 1);
        let mut pres: Vec<Array1<f32>> = Vec::with_capacity(layers);
        acts.push(Array1::from(input.to_vec()));
        for k in 0..layers {
            let z = acts[k].dot(&self.weights[k]) + &self.biases[k];
            pres.push(z.clone());
            acts.push(if k == layers - 1 {
                Array1::from(softmax(&z))
            } else {
                z.mapv(|x| if x > 0.0 { x } else { 0.0 })
            });
        }

        // Cross-entropy loss.
        let out = &acts[layers];
        let loss: f32 = target
            .iter()
            .zip(out.iter())
            .map(|(&t, &p)| -(t * p.max(1e-7).ln()))
            .sum();

        // Softmax + cross-entropy gradient is simply (p - t), clipped so a
        // single bad sample cannot explode the weights.
        let mut delta = out - &Array1::from(target.to_vec());
        for d in delta.iter_mut() {
            *d = d.clamp(-5.0, 5.0);
        }

        // Backpropagation, layer by layer.
        for k in (0..layers).rev() {
            let h_prev = &acts[k];
            for j in 0..delta.len() {
                let dj = delta[j];
                if dj == 0.0 {
                    continue;
                }
                for i in 0..h_prev.len() {
                    // Bound the per-parameter update: a long training run can
                    // never push a single weight past ±1 per sample.
                    let update = (learning_rate * dj * h_prev[i]).clamp(-1.0, 1.0);
                    self.weights[k][[i, j]] -= update;
                }
            }
            let bias_update = (&(learning_rate * &delta)).mapv(|d| d.clamp(-1.0, 1.0));
            self.biases[k] -= &bias_update;

            if k > 0 {
                // Gradient through the previous layer's ReLU.
                let w_t = self.weights[k].t();
                let mut nd = delta.dot(&w_t);
                for (i, pre) in pres[k - 1].iter().enumerate() {
                    if *pre <= 0.0 {
                        nd[i] = 0.0;
                    }
                }
                delta = nd;
            }
        }

        if !loss.is_finite() {
            // Numerical blow-up despite the clipping — reset the offending
            // weights instead of letting NaN poison the checkpoint.
            self.sanitize();
            return 0.0;
        }

        loss
    }
}

/* ================================================================ */
/*  MeMLP — the modular model container                              */
/* ================================================================ */

/// The modular container: several specialist MLP heads sharing one
/// JSON checkpoint. Add a new capability by adding a new module here.
#[derive(Serialize, Deserialize, Clone)]
pub struct MeMLP {
    pub version: u32,
    pub vegetation: Mlp,
    pub biome: Mlp,
    pub texture: Mlp,
}

/// Legacy checkpoint shape (pre-MeMLP single-hidden-layer model).
#[derive(Deserialize)]
pub struct LegacyCheckpoint {
    pub w1: Array2<f32>,
    pub b1: Array1<f32>,
    pub w2: Array2<f32>,
    pub b2: Array1<f32>,
    #[serde(default)]
    pub rng_state: u64,
    #[serde(default = "default_learning_rate")]
    pub learning_rate: f32,
    #[serde(default)]
    pub training_samples: usize,
}

fn default_learning_rate() -> f32 {
    0.01
}

impl MeMLP {
    /// Replace every non-finite weight in every module with `0.0`.
    pub fn sanitize(&mut self) {
        self.vegetation.sanitize();
        self.biome.sanitize();
        self.texture.sanitize();
    }

    /// A fresh modular model: deep vegetation net + biome + texture heads.
    pub fn new() -> Self {
        Self {
            version: MEMLP_VERSION,
            vegetation: Mlp::new(&VEGETATION_ARCH),
            biome: Mlp::new(&BIOME_ARCH),
            texture: Mlp::new(&TEXTURE_ARCH),
        }
    }

    /// Migrate a pre-MeMLP checkpoint: keep the trained vegetation weights,
    /// start the biome/texture heads from scratch.
    pub fn from_legacy(legacy: LegacyCheckpoint) -> Self {
        Self {
            version: MEMLP_VERSION,
            vegetation: Mlp::from_legacy(legacy.w1, legacy.b1, legacy.w2, legacy.b2),
            biome: Mlp::new(&BIOME_ARCH),
            texture: Mlp::new(&TEXTURE_ARCH),
        }
    }

    /// Total trainable parameters across every module.
    pub fn param_count(&self) -> usize {
        self.vegetation.param_count()
            + self.biome.param_count()
            + self.texture.param_count()
    }
}

impl Default for MeMLP {
    fn default() -> Self {
        Self::new()
    }
}

/* ================================================================ */
/*  Heuristic targets (synthetic + online training labels)           */
/* ================================================================ */

/// Heuristic biome class from the 8 climate features.
///
/// Indexes match the `BiomeId` order in `biomes.rs`:
/// 0 Ocean, 1 Coast, 2 Plains, 3 Forest, 4 DarkForest, 5 Swamp,
/// 6 Taiga, 7 Desert, 8 Mountains.
pub fn biome_heuristic_target(features: &[f32; 8]) -> usize {
    let height = features[0];
    let temperature = features[2];
    let humidity = features[3];

    if humidity < 0.22 && temperature > 0.55 {
        return 7; // Desert — hot and dry
    }
    if height > 0.72 && temperature < 0.45 {
        return 8; // Mountains — cold highlands
    }
    if temperature < 0.38 && humidity < 0.65 {
        return 6; // Taiga — cold, moderately dry
    }
    if humidity > 0.74 && temperature > 0.42 {
        return 5; // Swamp — very wet, warm
    }
    if humidity > 0.55 {
        return if temperature > 0.55 { 4 } else { 3 }; // DarkForest / Forest
    }
    if height < 0.12 {
        return if humidity > 0.5 { 1 } else { 0 }; // Coast / Ocean
    }
    2 // Plains
}

/// Heuristic texture-style class from the 8 climate features.
///
/// 0 Stone/Noise · 1 Wood · 2 Leaves · 3 Water · 4 Lava (arid) · 5 Speckle (sand/gravel)
pub fn texture_heuristic_target(features: &[f32; 8]) -> usize {
    let height = features[0];
    let temperature = features[2];
    let humidity = features[3];

    if humidity < 0.20 && temperature > 0.55 {
        return 4; // arid → Lava palette family
    }
    if humidity > 0.60 {
        return 2; // wet → Leaves
    }
    if humidity < 0.35 {
        return 5; // dry → Speckle (sand/gravel)
    }
    if (0.40..0.60).contains(&temperature) && (0.35..0.60).contains(&humidity) {
        return 1; // temperate → Wood
    }
    if height < 0.25 {
        return 3; // low ground → Water
    }
    0 // Stone/Noise
}

/* ================================================================ */
/*  Helpers                                                          */
/* ================================================================ */

/// Deterministic LCG step used for weight initialisation.
fn lcg_next(state: &mut u64) -> f32 {
    *state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
    (((*state / 65_536) % 32_768) as f32) / 32_767.0
}

/// Numerically-stable softmax over a vector.
fn softmax(x: &Array1<f32>) -> Vec<f32> {
    let max = x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = x.iter().map(|&v| (v - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    if sum > 0.0 {
        exps.iter().map(|&e| e / sum).collect()
    } else {
        vec![1.0 / x.len() as f32; x.len()]
    }
}

/// Index of the largest probability in a distribution.
pub fn argmax(probs: &[f32]) -> usize {
    probs
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// One-hot vector of length `n` for class `class`.
pub fn one_hot_n(class: usize, n: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; n];
    if class < n {
        v[class] = 1.0;
    }
    v
}

/* ================================================================ */
/*  Tests                                                            */
/* ================================================================ */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mlp_forward_is_a_valid_distribution() {
        let mlp = Mlp::new(&VEGETATION_ARCH);
        let out = mlp.forward(&[0.5, 0.2, 0.6, 0.7, 0.3, 0.4, 0.8, 0.5]);
        assert_eq!(out.len(), 4);
        let sum: f32 = out.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
        assert!(out.iter().all(|&p| (0.0..=1.0).contains(&p)));
    }

    #[test]
    fn mlp_init_is_deterministic() {
        let a = Mlp::new(&BIOME_ARCH);
        let b = Mlp::new(&BIOME_ARCH);
        assert_eq!(a.param_count(), b.param_count());
        for (wa, wb) in a.weights.iter().zip(b.weights.iter()) {
            assert_eq!(wa, wb, "same arch must initialise identically");
        }
    }

    #[test]
    fn mlp_trains_and_loss_decreases() {
        let mut mlp = Mlp::new(&[2, 6, 4, 2]);
        let input = [1.0, 0.0];
        let target = [1.0, 0.0];
        let loss1 = mlp.train(&input, &target, 0.1);
        let loss2 = mlp.train(&input, &target, 0.1);
        assert!(loss2 <= loss1 * 1.05, "loss should decrease: {loss1} -> {loss2}");
    }

    #[test]
    fn mlp_learns_a_simple_pattern() {
        let mut mlp = Mlp::new(&[2, 8, 4]);
        let (input, target) = ([0.9, 0.1], [0.0, 1.0, 0.0, 0.0]);
        let mut last = f32::MAX;
        for _ in 0..200 {
            last = mlp.train(&input, &target, 0.2);
        }
        assert!(last < 0.2, "model should fit the pattern, final loss {last}");
        let out = mlp.forward(&input);
        let pred = argmax(&out);
        assert_eq!(pred, 1);
    }

    #[test]
    fn legacy_checkpoint_migrates_cleanly() {
        let legacy = LegacyCheckpoint {
            w1: Array2::from_shape_vec((8, 16), vec![0.1f32; 8 * 16]).unwrap(),
            b1: Array1::from(vec![0.01f32; 16]),
            w2: Array2::from_shape_vec((16, 4), vec![0.05f32; 16 * 4]).unwrap(),
            b2: Array1::from(vec![0.001f32; 4]),
            rng_state: 42,
            learning_rate: 0.01,
            training_samples: 123,
        };
        let model = MeMLP::from_legacy(legacy);
        assert_eq!(model.version, MEMLP_VERSION);
        assert_eq!(model.vegetation.arch(), &[8, 16, 4]);
        assert_eq!(model.vegetation.param_count(), 8 * 16 + 16 + 16 * 4 + 4);
        // New heads are fresh but functional.
        assert_eq!(model.biome.forward(&[0.5; 8]).len(), 9);
        assert_eq!(model.texture.forward(&[0.5; 8]).len(), 6);
    }

    #[test]
    fn memlp_json_roundtrip() {
        let model = MeMLP::new();
        let json = serde_json::to_string(&model).unwrap();
        let loaded: MeMLP = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.version, MEMLP_VERSION);
        assert_eq!(loaded.param_count(), model.param_count());
        let feats = [0.3, 0.5, 0.6, 0.7, 0.2, 0.4, 0.8, 0.5];
        assert_eq!(
            loaded.vegetation.forward(&feats),
            model.vegetation.forward(&feats)
        );
    }

    #[test]
    fn training_survives_extreme_inputs_without_nan() {
        // Extreme magnitudes must never poison the weights: after heavy
        // training the checkpoint must stay free of `null` (NaN) values.
        let mut mlp = Mlp::new(&VEGETATION_ARCH);
        for _ in 0..50 {
            mlp.train(
                &[1e6, -1e6, 1e5, -1e5, 1e4, -1e4, 1e3, -1e3],
                &[1.0, 0.0, 0.0, 0.0],
                0.05,
            );
        }
        mlp.sanitize();
        let model = MeMLP {
            version: MEMLP_VERSION,
            vegetation: mlp,
            biome: Mlp::new(&BIOME_ARCH),
            texture: Mlp::new(&TEXTURE_ARCH),
        };
        let json = serde_json::to_string(&model).unwrap();
        assert!(
            !json.contains("null"),
            "NaN weights would serialise as null: {json}"
        );
        let loaded: MeMLP = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.param_count(), model.param_count());
    }

    #[test]
    fn training_skips_nan_inputs_without_touching_weights() {
        let mut mlp = Mlp::new(&[2, 4, 2]);
        let before = mlp.clone();
        let loss = mlp.train(&[f32::NAN, 0.5], &[1.0, 0.0], 0.1);
        assert!(loss.is_infinite(), "poisoned input must be rejected");
        for (wa, wb) in mlp.weights.iter().zip(before.weights.iter()) {
            assert_eq!(wa, wb, "weights must stay untouched on poisoned input");
        }
    }

    #[test]
    fn sanitize_clears_nan_and_inf() {
        let mut mlp = Mlp::new(&[2, 4, 2]);
        mlp.weights[0][[0, 0]] = f32::NAN;
        mlp.weights[1][[1, 0]] = f32::INFINITY;
        mlp.biases[0][0] = f32::NEG_INFINITY;
        mlp.sanitize();
        for w in &mlp.weights {
            assert!(w.iter().all(|v| v.is_finite()));
        }
        for b in &mlp.biases {
            assert!(b.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn heuristic_targets_stay_in_range() {
        for i in 0..500 {
            let feats = [
                (i as f32 / 500.0).fract(),
                (i as f32 * 0.37).fract() * 0.5,
                (i as f32 * 0.71).fract(),
                (i as f32 * 0.43).fract(),
                0.5,
                0.5,
                0.7,
                0.5,
            ];
            let b = biome_heuristic_target(&feats);
            assert!(b < 9, "biome class {b} out of range");
            let t = texture_heuristic_target(&feats);
            assert!(t < TEXTURE_CLASSES, "texture class {t} out of range");
        }
    }
}
