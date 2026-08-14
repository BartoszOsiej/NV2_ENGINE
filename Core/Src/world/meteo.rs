//! Real-world climate data for NV-2.0 chunk generation — **NASA POWER API**.
//!
//! Every world seed maps deterministically to a point on Earth
//! ([`world_coordinates`]). At world creation the engine tries to fetch the
//! annual climate for that point (2m temperature, precipitation, humidity,
//! all-sky solar radiation and wind) from the NASA POWER REST API. The
//! generator then blends this *real* regional baseline into the biome
//! climate (temperature / humidity / weather), so terrain, vegetation and
//! weather genuinely reflect the real world at that location.
//!
//! If the network is unavailable (offline launch, sandbox, CI) the engine
//! falls back to a **deterministic synthetic climate** derived from the
//! seed, so the world is stable per seed and the game never depends on a
//! live connection.

use std::sync::OnceLock;

/// Annual climate snapshot for a world location.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeteoData {
    /// Annual mean 2-metre air temperature, °C.
    pub temperature_c: f64,
    /// Annual mean daily precipitation, mm/day.
    pub precipitation_mm: f64,
    /// Annual mean relative humidity, %.
    pub humidity_pct: f64,
    /// Annual mean all-sky surface solar radiation, kWh/m²/day.
    pub solar_kwh: f64,
    /// Annual mean wind speed at 2 m, m/s.
    pub wind_ms: f64,
}

impl MeteoData {
    /// Normalised warmth in 0..1 (≈ the engine's biome `temperature` axis).
    /// Maps ≈ -12 °C → 0.0 to ≈ +30 °C → 1.0.
    pub fn warmth(&self) -> f32 {
        ((self.temperature_c + 12.0) / 42.0).clamp(0.0, 1.0) as f32
    }

    /// Normalised moisture in 0..1 (≈ the engine's biome `humidity` axis).
    /// 12 mm/day of annual-mean precipitation ≈ fully saturated.
    pub fn moisture(&self) -> f32 {
        (self.precipitation_mm / 12.0).clamp(0.0, 1.0) as f32
    }

    /// Normalised humidity in 0..1 (for fog / cloud density).
    pub fn humidity(&self) -> f32 {
        (self.humidity_pct / 100.0).clamp(0.0, 1.0) as f32
    }

    /// Cloud cover 0..1 from humidity + precipitation + solar deficit.
    pub fn cloud_cover(&self) -> f32 {
        let wet = self.moisture();
        let hum = self.humidity();
        let dark = ((1.0 - (self.solar_kwh / 7.0).clamp(0.0, 1.0)) * 0.4) as f32;
        (wet * 0.45 + hum * 0.35 + dark).clamp(0.0, 1.0)
    }

    /// True when precipitation falls as snow (cold climate).
    pub fn is_snow(&self) -> bool {
        self.precipitation_mm > 0.5 && self.temperature_c < 0.5
    }

    /// True when precipitation falls as rain (mild/hot climate).
    pub fn is_rain(&self) -> bool {
        self.precipitation_mm > 1.0 && self.temperature_c >= 0.5
    }

    /// Short weather label for the HUD.
    pub fn weather_label(&self) -> &'static str {
        if self.is_snow() {
            "❄ snow"
        } else if self.is_rain() {
            "🌧 rain"
        } else if self.humidity() > 0.75 {
            "☁ cloudy"
        } else if self.wind_ms > 8.0 {
            "💨 windy"
        } else {
            "☀ clear"
        }
    }
}

/// Deterministic mapping from a world seed to a point on Earth.
/// Longitude spans -180..180; latitude spans -60..+70 (avoiding the
/// permanent ice caps so every seed yields a playable climate).
pub fn world_coordinates(seed: u32) -> (f64, f64) {
    let lon = (seed % 360_000) as f64 / 1000.0 - 180.0;
    let lat = 70.0 - ((seed / 360_000) % 130_000) as f64 / 1000.0;
    (lat, lon)
}

/// Process-wide cache: a session fetches real climate at most once.
static METEO_CACHE: OnceLock<MeteoData> = OnceLock::new();
/// True when the cached climate came from NASA POWER (not the fallback).
static METEO_REAL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Whether the current session's climate is real NASA POWER data.
pub fn is_real_meteo() -> bool {
    METEO_REAL.load(std::sync::atomic::Ordering::Relaxed)
}

/// Climate for a seed: real NASA POWER data when reachable, otherwise a
/// deterministic synthetic climate so worlds stay stable offline.
///
/// The process-wide cache exists only so a session never refetches the
/// network. Tests bypass it entirely — every seed gets its own synthetic
/// climate, so test outcomes never depend on run order.
pub fn meteo_for_seed(seed: u32) -> MeteoData {
    #[cfg(test)]
    {
        synthetic(seed)
    }
    #[cfg(not(test))]
    {
        *METEO_CACHE.get_or_init(|| {
            let (lat, lon) = world_coordinates(seed);
            match fetch_nasa_power(lat, lon) {
                Some(real) => {
                    METEO_REAL.store(true, std::sync::atomic::Ordering::Relaxed);
                    real
                }
                None => synthetic(seed),
            }
        })
    }
}

/// Query the NASA POWER API for the annual climate at (lat, lon).
///
/// Endpoint: `power.larc.nasa.gov/api/temporal/daily/point` with the
/// renewable-energy (`RE`) community parameters. We average the whole
/// requested year so one number per parameter feeds generation.
pub fn fetch_nasa_power(lat: f64, lon: f64) -> Option<MeteoData> {
    let url = format!(
        "https://power.larc.nasa.gov/api/temporal/daily/point?\
         parameters=T2M,PRECTOTCORR,RH2M,ALLSKY_SFC_SW_DWN,WS2M&\
         community=RE&longitude={lon:.3}&latitude={lat:.3}&\
         start=20230101&end=20231231&format=JSON"
    );
    let fut = async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .ok()?;
        let resp = client.get(&url).send().await.ok()?;
        let json: serde_json::Value = resp.json().await.ok()?;
        let params = json.pointer("/properties/parameter")?;
        let annual_mean = |key: &str| -> Option<f64> {
            let map = params.get(key)?.as_object()?;
            let mut sum = 0.0;
            let mut n = 0.0;
            for v in map.values() {
                if let Some(x) = v.as_f64() {
                    if x.is_finite() {
                        sum += x;
                        n += 1.0;
                    }
                }
            }
            if n == 0.0 {
                None
            } else {
                Some(sum / n)
            }
        };
        Some(MeteoData {
            temperature_c: annual_mean("T2M")?,
            precipitation_mm: annual_mean("PRECTOTCORR")?,
            humidity_pct: annual_mean("RH2M")?,
            solar_kwh: annual_mean("ALLSKY_SFC_SW_DWN")?,
            wind_ms: annual_mean("WS2M")?,
        })
    };
    // reqwest 0.11 needs a real tokio reactor — pollster's block_on does
    // not provide one, so spin up a short-lived current-thread runtime.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    rt.block_on(fut)
}

/// Deterministic pseudo-climate derived from the seed — the offline and
/// test fallback. It is centred on a temperate distribution (warmth and
/// moisture ≈ 0.4..0.7) so worlds look like the classic procedural ones.
fn synthetic(seed: u32) -> MeteoData {
    let h = |x: u32| -> f64 {
        let mut a = x.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
        a ^= a >> 13;
        a = a.wrapping_mul(0xC2B2_AE35);
        a ^= a >> 16;
        (a & 0xFFFF) as f64 / 65_535.0
    };
    // A neutral climate (warmth/moisture ≈ 0.5) so the shift applied by
    // [`MeteoData::warmth`] / [`MeteoData::moisture`] is ≈ 0 and offline
    // worlds keep the classic noise-driven biome distribution.
    let temperature_c = (9.0 + (h(seed) - 0.5) * 8.0).clamp(-5.0, 20.0);
    let precipitation_mm = (6.0 + (h(seed ^ 0xA5A5) - 0.5) * 5.0).clamp(1.0, 10.0);
    let humidity_pct = (45.0 + h(seed ^ 0x3C3C) * 45.0).clamp(10.0, 98.0);
    let solar_kwh = (2.0 + h(seed ^ 0x5A5A) * 4.0).clamp(0.5, 7.5);
    let wind_ms = (1.5 + h(seed ^ 0x0F0F) * 9.0).clamp(0.5, 12.0);
    MeteoData {
        temperature_c,
        precipitation_mm,
        humidity_pct,
        solar_kwh,
        wind_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_coordinates_are_deterministic_and_bounded() {
        for seed in [0_u32, 1, 42, 1337, 7_654_321, u32::MAX] {
            let (lat, lon) = world_coordinates(seed);
            assert!((-60.0..=70.0).contains(&lat), "lat {lat} for seed {seed}");
            assert!((-180.0..=180.0).contains(&lon), "lon {lon} for seed {seed}");
            let (lat2, lon2) = world_coordinates(seed);
            assert_eq!((lat, lon), (lat2, lon2));
        }
    }

    #[test]
    fn distinct_seeds_give_distinct_locations() {
        let (la1, lo1) = world_coordinates(1);
        let (la2, lo2) = world_coordinates(2);
        assert!((la1 - la2).abs() > 1e-9 || (lo1 - lo2).abs() > 1e-9);
    }

    #[test]
    fn synthetic_climate_is_sane_and_stable() {
        for seed in [7_u32, 99, 123_456] {
            let m = meteo_for_seed(seed);
            assert!(m.temperature_c >= -12.0 && m.temperature_c <= 32.0);
            assert!(m.precipitation_mm >= 0.1 && m.precipitation_mm <= 12.0);
            assert!(m.humidity_pct >= 10.0 && m.humidity_pct <= 98.0);
            assert!(m.warmth() >= 0.0 && m.warmth() <= 1.0);
            assert!(m.moisture() >= 0.0 && m.moisture() <= 1.0);
            assert!(m.cloud_cover() >= 0.0 && m.cloud_cover() <= 1.0);
            assert!(!m.weather_label().is_empty());
        }
    }

    #[test]
    fn warmth_moisture_mapping() {
        let hot = MeteoData {
            temperature_c: 30.0,
            precipitation_mm: 12.0,
            humidity_pct: 90.0,
            solar_kwh: 6.0,
            wind_ms: 2.0,
        };
        assert_eq!(hot.warmth(), 1.0);
        assert_eq!(hot.moisture(), 1.0);
        assert!(hot.is_rain());
        assert!(!hot.is_snow());

        let cold_dry = MeteoData {
            temperature_c: -30.0,
            precipitation_mm: 0.0,
            humidity_pct: 30.0,
            solar_kwh: 1.0,
            wind_ms: 10.0,
        };
        assert_eq!(cold_dry.warmth(), 0.0);
        assert_eq!(cold_dry.moisture(), 0.0);
        assert!(!cold_dry.is_rain());
        assert!(!cold_dry.is_snow()); // precipitation too low for snow

        let snowy = MeteoData {
            temperature_c: -5.0,
            precipitation_mm: 3.0,
            humidity_pct: 80.0,
            solar_kwh: 1.5,
            wind_ms: 6.0,
        };
        assert!(snowy.is_snow());
        assert_eq!(snowy.weather_label(), "❄ snow");
    }
}
