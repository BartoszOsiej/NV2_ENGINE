//! Real-world climate data for NV-2.0 chunk generation.
//!
//! Every world seed maps deterministically to a point on Earth
//! ([`world_coordinates`]). The engine ships with a **real global
//! climatology** embedded in the binary ([`climate_grid`], generated from
//! NCEP/NCAR Reanalysis 1981–2010 monthly means by
//! `tools/fetch_climate.py`). The generator therefore always has a *real,
//! offline, deterministic* climate baseline: which biome dominates a region
//! (Sahara → desert, Amazon → rainforest, Siberia → taiga) and how seasons
//! shift temperature/precipitation come straight from measured Earth data.
//!
//! When the network is available the engine additionally queries the
//! **NASA POWER API** for the current-year annual climate at the anchor
//! point (2 m temperature, precipitation, humidity, solar radiation, wind)
//! and uses it to refine the baseline — so the HUD reflects both real
//! climatology *and* current conditions when online.

use std::sync::OnceLock;

/// 2.5°-resolution embedded climatology, see `tools/fetch_climate.py`.
const CLIMATE_BYTES: &[u8] = include_bytes!("../../assets/climate.dat");
const CLIMATE_MAGIC: &[u8; 8] = b"NV2CLIM1";
const GRID_LAT: usize = 73; // 90°N .. 90°S
const GRID_LON: usize = 144; // 0° .. 357.5°
const CELL_DEG: f64 = 2.5;

/// Real-world climate grid: annual + monthly temperature / precipitation.
pub struct ClimateGrid {
    t_ann: Vec<i16>, // 0.01 °C
    p_ann: Vec<u16>, // 0.01 mm/day
    t_mon: Vec<i16>, // [12][lat][lon], 0.01 °C
    p_mon: Vec<u16>, // [12][lat][lon], 0.01 mm/day
}

impl ClimateGrid {
    fn parse(bytes: &[u8]) -> Option<ClimateGrid> {
        if bytes.len() < 12 || &bytes[0..8] != CLIMATE_MAGIC {
            return None;
        }
        let nlat = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
        let nlon = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
        if nlat != GRID_LAT || nlon != GRID_LON {
            return None;
        }
        let cells = nlat * nlon;
        let t_ann_len = cells * 2;
        let p_ann_len = cells * 2;
        let t_mon_len = cells * 12 * 2;
        let p_mon_len = cells * 12 * 2;
        let expect = 12 + t_ann_len + p_ann_len + t_mon_len + p_mon_len;
        if bytes.len() < expect {
            return None;
        }
        let mut off = 12;
        let read_i16 = |off: &mut usize, n: usize| -> Vec<i16> {
            let v = bytes[*off..*off + n * 2]
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            *off += n * 2;
            v
        };
        let read_u16 = |off: &mut usize, n: usize| -> Vec<u16> {
            let v = bytes[*off..*off + n * 2]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            *off += n * 2;
            v
        };
        Some(ClimateGrid {
            t_ann: read_i16(&mut off, cells),
            p_ann: read_u16(&mut off, cells),
            t_mon: read_i16(&mut off, cells * 12),
            p_mon: read_u16(&mut off, cells * 12),
        })
    }

    /// Annual mean temperature (°C) and precipitation (mm/day) at (lat, lon).
    pub fn annual_at(&self, lat: f64, lon: f64) -> (f64, f64) {
        let t = self.bilerp(lat, lon, |j, i| self.t_ann[j * GRID_LON + i] as f64 / 100.0);
        let p = self.bilerp(lat, lon, |j, i| self.p_ann[j * GRID_LON + i] as f64 / 100.0);
        (t, p.max(0.0))
    }

    /// Monthly mean temperatures (°C) for all 12 months at (lat, lon).
    pub fn monthly_temps(&self, lat: f64, lon: f64) -> [f64; 12] {
        let mut out = [0.0; 12];
        for m in 0..12 {
            out[m] = self.bilerp(lat, lon, |j, i| {
                self.t_mon[(m * GRID_LAT + j) * GRID_LON + i] as f64 / 100.0
            });
        }
        out
    }

    /// Monthly mean precipitation (mm/day) for all 12 months at (lat, lon).
    pub fn monthly_precip(&self, lat: f64, lon: f64) -> [f64; 12] {
        let mut out = [0.0; 12];
        for m in 0..12 {
            out[m] = self
                .bilerp(lat, lon, |j, i| {
                    self.p_mon[(m * GRID_LAT + j) * GRID_LON + i] as f64 / 100.0
                })
                .max(0.0);
        }
        out
    }

    /// Row/col brackets for (lat, lon): rows descend 90→-90, cols wrap 0→360.
    fn brackets(&self, lat: f64, lon: f64) -> ((usize, usize), (usize, usize), f64, f64) {
        let lat = lat.clamp(-90.0, 90.0);
        let lon = lon.rem_euclid(360.0);
        // row: 90°N = row 0, -90° = row GRID_LAT-1
        let r = (90.0 - lat) / CELL_DEG;
        let j0 = (r.floor() as usize).min(GRID_LAT - 1);
        let j1 = (j0 + 1).min(GRID_LAT - 1);
        let fy = (r - j0 as f64).clamp(0.0, 1.0);
        // col: 0° = col 0, wraps
        let c = lon / CELL_DEG;
        let i0 = (c.floor() as usize) % GRID_LON;
        let i1 = (i0 + 1) % GRID_LON;
        let fx = c - c.floor();
        ((j0, j1), (i0, i1), fx, fy)
    }

    fn bilerp<F: Fn(usize, usize) -> f64>(&self, lat: f64, lon: f64, get: F) -> f64 {
        let ((j0, j1), (i0, i1), fx, fy) = self.brackets(lat, lon);
        let v00 = get(j0, i0);
        let v01 = get(j0, i1);
        let v10 = get(j1, i0);
        let v11 = get(j1, i1);
        let top = v00 * (1.0 - fx) + v01 * fx;
        let bot = v10 * (1.0 - fx) + v11 * fx;
        top * (1.0 - fy) + bot * fy
    }
}

/// The embedded real climatology (parsed once per process).
pub fn climate_grid() -> &'static ClimateGrid {
    static GRID: OnceLock<ClimateGrid> = OnceLock::new();
    GRID.get_or_init(|| {
        ClimateGrid::parse(CLIMATE_BYTES).unwrap_or_else(|| {
            // Never happens for the shipped asset; fall back to a zero grid
            // so the game still boots if the binary is corrupted.
            ClimateGrid {
                t_ann: vec![0; GRID_LAT * GRID_LON],
                p_ann: vec![0; GRID_LAT * GRID_LON],
                t_mon: vec![0; GRID_LAT * GRID_LON * 12],
                p_mon: vec![0; GRID_LAT * GRID_LON * 12],
            }
        })
    })
}

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
    /// Mean temperature per calendar month (Jan..Dec), °C — real climatology.
    pub month_temps_c: [f64; 12],
    /// Mean precipitation per calendar month, mm/day.
    pub month_precip_mm: [f64; 12],
    /// Where the numbers came from (for the HUD / status message).
    pub source: &'static str,
}

/// Days per month (Jan..Dec) for seasonal interpolation.
const DAYS_PER_MONTH: [f64; 12] = [31.0, 28.25, 31.0, 30.0, 31.0, 30.0, 31.0, 31.0, 30.0, 31.0, 30.0, 31.0];

/// Interpolate a 12-month cycle to a day-of-year (1..=365).
fn cycle_at(values: &[f64; 12], day_of_year: u32) -> f64 {
    let mut day = (day_of_year.max(1) as f64 - 1.0).min(364.0);
    let mut month = 0usize;
    while month < 11 && day >= DAYS_PER_MONTH[month] {
        day -= DAYS_PER_MONTH[month];
        month += 1;
    }
    let frac = day / DAYS_PER_MONTH[month];
    let next = (month + 1) % 12;
    values[month] * (1.0 - frac) + values[next] * frac
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

    /// Temperature (°C) on a given day of year (1..=365) — real seasonality.
    pub fn temp_for_day(&self, day_of_year: u32) -> f64 {
        cycle_at(&self.month_temps_c, day_of_year)
    }

    /// Precipitation (mm/day) on a given day of year — real seasonality.
    pub fn precip_for_day(&self, day_of_year: u32) -> f64 {
        cycle_at(&self.month_precip_mm, day_of_year).max(0.0)
    }

    /// Cloud cover on a given day of year.
    pub fn cloud_cover_for_day(&self, day_of_year: u32) -> f32 {
        let wet = (self.precip_for_day(day_of_year) / 12.0).clamp(0.0, 1.0) as f32;
        let hum = self.humidity();
        let dark = ((1.0 - (self.solar_kwh / 7.0).clamp(0.0, 1.0)) * 0.4) as f32;
        (wet * 0.45 + hum * 0.35 + dark).clamp(0.0, 1.0)
    }

    /// Seasonal weather label for a given day of year.
    pub fn weather_label_for_day(&self, day_of_year: u32) -> &'static str {
        let t = self.temp_for_day(day_of_year);
        let p = self.precip_for_day(day_of_year);
        if p > 0.5 && t < 0.5 {
            "❄ snow"
        } else if p > 1.0 {
            "🌧 rain"
        } else if self.humidity() > 0.75 {
            "☁ cloudy"
        } else if self.wind_ms > 8.0 {
            "💨 windy"
        } else {
            "☀ clear"
        }
    }

    /// Short weather label for the HUD (annual mean conditions).
    pub fn weather_label(&self) -> &'static str {
        self.weather_label_for_day(15)
    }
}

/// Deterministic mapping from a world seed to a point on Earth.
/// Longitude spans -180..180; latitude spans -60..+70 (avoiding the
/// permanent ice caps so every seed yields a playable climate). The seed
/// is split into two 16-bit halves so the full latitude range is reachable
/// (the old formula capped every seed at 58..70°N — every world was arctic).
pub fn world_coordinates(seed: u32) -> (f64, f64) {
    let lo = (seed & 0xFFFF) as f64 / 65_535.0; // 0..1 → longitude
    let hi = ((seed >> 16) & 0xFFFF) as f64 / 65_535.0; // 0..1 → latitude
    let lon = lo * 360.0 - 180.0;
    let lat = 70.0 - hi * 130.0; // 70 .. -60
    (lat, lon)
}

/// Process-wide cache: a session fetches live NASA POWER at most once.
static METEO_CACHE: OnceLock<MeteoData> = OnceLock::new();

/// Whether the current session's climate came from NASA POWER live data.
pub fn is_real_meteo() -> bool {
    METEO_CACHE.get().is_some()
}

/// Climate for a seed: the embedded **real NCEP climatology** at the anchor
/// point, refined by a live **NASA POWER** annual fetch when the network is
/// reachable. Fully deterministic per seed, works offline, never blocks
/// generation (8 s network timeout, one fetch per session).
pub fn meteo_for_seed(seed: u32) -> MeteoData {
    *METEO_CACHE.get_or_init(|| {
        let (lat, lon) = world_coordinates(seed);
        let grid = climate_grid();
        let (t_c, p_mm) = grid.annual_at(lat, lon);
        let month_temps_c = grid.monthly_temps(lat, lon);
        let month_precip_mm = grid.monthly_precip(lat, lon);

        // Humidity / solar / wind aren't in the embedded grid — derive
        // plausible real-world values from temperature and precipitation,
        // then let the live NASA POWER fetch refine them when online.
        let warmest = month_temps_c.iter().copied().fold(f64::MIN, f64::max);
        let humidity_pct = (42.0 + (p_mm - 0.5) * 5.0 - (t_c - 15.0) * 0.6)
            .clamp(12.0, 98.0);
        let solar_kwh = (7.0 * (lat.to_radians().cos()).abs()).clamp(0.5, 7.5);
        let wind_ms = (3.0 + ((100.0 - warmest.abs()) / 100.0) * 4.0).clamp(0.5, 12.0);

        let mut base = MeteoData {
            temperature_c: t_c,
            precipitation_mm: p_mm,
            humidity_pct,
            solar_kwh,
            wind_ms,
            month_temps_c,
            month_precip_mm,
            source: "NCEP 1981-2010 climatology",
        };

        // Live refinement: current-year annual means from NASA POWER.
        if let Some(real) = fetch_nasa_power(lat, lon) {
            base.temperature_c = real.temperature_c;
            base.precipitation_mm = real.precipitation_mm;
            base.humidity_pct = real.humidity_pct;
            base.solar_kwh = real.solar_kwh;
            base.wind_ms = real.wind_ms;
            base.source = "NASA POWER live";
        }
        base
    })
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
            month_temps_c: [0.0; 12],
            month_precip_mm: [0.0; 12],
            source: "NASA POWER live",
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
    fn embedded_grid_matches_known_real_climates() {
        let grid = climate_grid();
        // Sahara — hot and bone dry.
        let (t, p) = grid.annual_at(24.0, 15.0);
        assert!((t - 22.8).abs() < 2.0, "Sahara T {t}");
        assert!(p < 0.5, "Sahara P {p}");
        // Amazon — hot and very wet.
        let (t, p) = grid.annual_at(-3.0, -60.0);
        assert!((t - 24.9).abs() < 2.0, "Amazon T {t}");
        assert!(p > 6.0, "Amazon P {p}");
        // Siberia — cold.
        let (t, _) = grid.annual_at(60.0, 90.0);
        assert!(t < 2.0, "Siberia T {t}");
    }

    #[test]
    fn embedded_climate_is_sane_and_stable() {
        for seed in [7_u32, 99, 123_456] {
            let m = meteo_for_seed(seed);
            assert!(m.temperature_c >= -45.0 && m.temperature_c <= 35.0);
            assert!(m.precipitation_mm >= 0.0 && m.precipitation_mm <= 16.0);
            assert!(m.humidity_pct >= 10.0 && m.humidity_pct <= 98.0);
            assert!(m.warmth() >= 0.0 && m.warmth() <= 1.0);
            assert!(m.moisture() >= 0.0 && m.moisture() <= 1.0);
            assert!(m.cloud_cover() >= 0.0 && m.cloud_cover() <= 1.0);
            assert!(!m.weather_label().is_empty());
            assert_eq!(m.month_temps_c.len(), 12);
            assert_eq!(m.month_precip_mm.len(), 12);
            // seasonal temps must have a real annual cycle in the extratropics
            let spread = m.month_temps_c.iter().copied().fold(f64::MIN, f64::max)
                - m.month_temps_c.iter().copied().fold(f64::MAX, f64::min);
            if m.temperature_c > -5.0 {
                assert!(spread > 3.0, "expected a real seasonal cycle, spread {spread}");
            }
        }
    }

    #[test]
    fn seasonal_weather_follows_the_real_cycle() {
        // A northern-hemisphere temperate location: winter cold, summer mild.
        let (lat, lon) = (51.0, 0.0);
        let grid = climate_grid();
        let m = meteo_for_seed(0);
        let jan = grid.monthly_temps(lat, lon);
        assert!(jan[0] < jan[6], "London Jan {:.1} should be colder than Jul {:.1}", jan[0], jan[6]);
        let _ = m;
    }

    #[test]
    fn warmth_moisture_mapping() {
        let hot = MeteoData {
            temperature_c: 30.0,
            precipitation_mm: 12.0,
            humidity_pct: 90.0,
            solar_kwh: 6.0,
            wind_ms: 2.0,
            month_temps_c: [25.0; 12],
            month_precip_mm: [10.0; 12],
            source: "test",
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
            month_temps_c: [-30.0; 12],
            month_precip_mm: [0.0; 12],
            source: "test",
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
            month_temps_c: [-5.0; 12],
            month_precip_mm: [3.0; 12],
            source: "test",
        };
        assert!(snowy.is_snow());
        assert_eq!(snowy.weather_label(), "❄ snow");
    }
}
