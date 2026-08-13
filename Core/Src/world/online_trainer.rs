//! Online training data for the AI vegetation system.
//!
//! Fetches real-world terrain/climate samples from a public, keyless API
//! (Open-Meteo — free, no account, CORS-friendly) and turns them into
//! (features, target) training samples for the vegetation network.
//!
//! **Offline fallback:** if the network is unavailable (or the request
//! times out), [`fetch_samples_blocking`] returns synthetic samples, so
//! training never stalls. The AI also never blocks on the network — this
//! module is only called from the background training loop.

use serde::Deserialize;

pub type TrainingSample = ([f32; 8], [f32; 4]);

/// Locations whose current weather becomes training data.
/// Picked to span very different biomes (desert, rainforest, tundra…).
const SAMPLE_LOCATIONS: &[(f32, f32)] = &[
    (52.2297, 21.0122), // Warsaw — temperate
    (-1.2864, 36.8172), // Nairobi — highland
    (25.2048, 55.2708), // Dubai — desert
    (-3.1190, -60.0217), // Amazonas — rainforest
    (64.1466, -21.9426), // Reykjavik — tundra
    (39.9042, 116.4074), // Beijing — continental
    (-33.8688, 151.2093), // Sydney — coastal
    (51.5074, -0.1278), // London — maritime
];

/// A single sample from the Open-Meteo API.
#[derive(Deserialize)]
struct OpenMeteoResponse {
    current: OpenMeteoCurrent,
}

#[derive(Deserialize)]
struct OpenMeteoCurrent {
    temperature_2m: f32,
    relative_humidity_2m: f32,
}

/// How many synthetic samples to generate when offline.
const SYNTHETIC_COUNT: usize = 64;

/// Heuristic target: same rules as `TerrainAI::target_vegetation` so online
/// data is consistent with synthetic training.
fn target_vegetation(features: &[f32; 8]) -> [f32; 4] {
    let height = features[0];
    let humidity = features[3];
    let light = features[6];

    let mut probs = [0.0f32; 4];

    if humidity > 0.5f32 && light > 0.5f32 {
        probs[0] = (humidity * 0.7f32 + light * 0.3f32).min(1.0f32);
    }
    if humidity > 0.7f32 && light < 0.4f32 {
        probs[1] = (humidity * 0.8f32).min(1.0f32);
    } else if probs[0] < 0.3f32 && probs[1] < 0.3f32 {
        probs[2] = 0.6f32;
    }
    if height < 0.2f32 {
        probs[3] = 0.7f32;
    }

    let sum: f32 = probs.iter().sum();
    if sum > 0.0f32 {
        for p in probs.iter_mut() {
            *p /= sum;
        }
    } else {
        probs[2] = 1.0f32;
    }
    probs
}

/// Fetch live weather for one location and convert to a sample.
async fn fetch_location(client: &reqwest::Client, lat: f32, lon: f32) -> Option<TrainingSample> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
         &current=temperature_2m,relative_humidity_2m&timezone=UTC"
    );
    let resp: OpenMeteoResponse = client.get(&url).send().await.ok()?.json().await.ok()?;

    // Normalise temperature (-40..50 °C) and humidity (0..100) to 0..1.
    let temperature = ((resp.current.temperature_2m + 40.0) / 90.0).clamp(0.0, 1.0);
    let humidity = (resp.current.relative_humidity_2m / 100.0).clamp(0.0, 1.0);

    // Map to the 8 AI features. Terrain height/slope stay generic; the
    // climate signals are the real-world part.
    let features: [f32; 8] = [
        0.5,           // terrain_height (generic)
        0.25,          // terrain_slope (generic)
        temperature,   // biome_temperature ← real
        humidity,      // biome_humidity ← real
        0.5,           // nearby_water_distance (generic)
        0.5,           // nearby_vegetation_count (generic)
        0.7,           // light_level (generic daytime)
        0.5,           // noise_seed_value
    ];

    let target = target_vegetation(&features);
    Some((features, target))
}

/// Fetch training samples from the network.
///
/// Returns `None` on any failure so the caller can fall back.
async fn fetch_online() -> Option<Vec<TrainingSample>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("NV2-Engine/0.1")
        .build()
        .ok()?;

    let mut out = Vec::new();
    for &(lat, lon) in SAMPLE_LOCATIONS {
        if let Some(sample) = fetch_location(&client, lat, lon).await {
            out.push(sample);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Generate synthetic samples (offline fallback / training stabiliser).
pub fn synthetic_samples() -> Vec<TrainingSample> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut out = Vec::with_capacity(SYNTHETIC_COUNT);
    for _ in 0..SYNTHETIC_COUNT {
        let features: [f32; 8] = [
            rng.gen::<f32>(),
            rng.gen::<f32>() * 0.5,
            rng.gen::<f32>(),
            rng.gen::<f32>(),
            rng.gen::<f32>(),
            rng.gen::<f32>(),
            rng.gen::<f32>(),
            rng.gen::<f32>(),
        ];
        let target = target_vegetation(&features);
        out.push((features, target));
    }
    out
}

/// Blocking wrapper for the online fetch.
///
/// * Online data if the request succeeds.
/// * Synthetic data otherwise (offline, timeout, rate-limit…).
pub fn fetch_samples_blocking() -> Vec<TrainingSample> {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return synthetic_samples(),
    };

    match rt.block_on(fetch_online()) {
        Some(samples) => samples,
        None => synthetic_samples(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_samples_have_valid_shape() {
        let samples = synthetic_samples();
        assert_eq!(samples.len(), SYNTHETIC_COUNT);
        for (features, target) in &samples {
            assert_eq!(features.len(), 8);
            assert_eq!(target.len(), 4);
            let sum: f32 = target.iter().sum();
            assert!((sum - 1.0).abs() < 0.01, "target must be a distribution");
        }
    }

    #[test]
    fn target_heuristic_matches_shape() {
        let features = [0.5, 0.25, 0.6, 0.8, 0.5, 0.5, 0.7, 0.5];
        let target = target_vegetation(&features);
        let sum: f32 = target.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }
}
