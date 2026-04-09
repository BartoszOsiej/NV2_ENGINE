use std::{
    env, fs,
    io::ErrorKind,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppSettings {
    pub low_end_pc: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { low_end_pc: false }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(settings) => settings,
                Err(err) => {
                    log::warn!("Failed to parse settings file '{}': {}", path.display(), err);
                    Self::default()
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound => Self::default(),
            Err(err) => {
                log::warn!("Failed to read settings file '{}': {}", path.display(), err);
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create settings directory '{}'", parent.display()))?;
        }

        let payload = serde_json::to_vec_pretty(self).context("serialize settings")?;
        fs::write(&path, payload)
            .with_context(|| format!("write settings file '{}'", path.display()))?;
        Ok(())
    }

    pub fn profile(self) -> PerformanceProfile {
        PerformanceProfile::from_low_end_pc(self.low_end_pc)
    }

    fn config_path() -> PathBuf {
        let base_dir = env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        base_dir.join("settings.json")
    }
}

#[derive(Clone, Debug)]
pub struct SharedSettings(Arc<RwLock<AppSettings>>);

impl SharedSettings {
    pub fn new(settings: AppSettings) -> Self {
        Self(Arc::new(RwLock::new(settings)))
    }

    pub fn snapshot(&self) -> AppSettings {
        *self.0.read().expect("settings lock poisoned")
    }

    pub fn low_end_pc(&self) -> bool {
        self.snapshot().low_end_pc
    }

    pub fn set_low_end_pc(&self, enabled: bool) {
        self.0.write().expect("settings lock poisoned").low_end_pc = enabled;
    }

    pub fn profile(&self) -> PerformanceProfile {
        self.snapshot().profile()
    }

    pub fn save(&self) -> Result<()> {
        self.snapshot().save()
    }
}

impl Default for SharedSettings {
    fn default() -> Self {
        Self::new(AppSettings::default())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerformanceProfile {
    pub load_radius: i32,
    pub render_radius: i32,
    pub cleanup_radius: i32,
    pub fog_density_multiplier: f32,
    pub mesh_build_budget: u32,
    pub water_sim_interval: f32,
    pub water_rebuild_interval: f32,
    pub prefer_vsync: bool,
    pub hide_dense_foliage: bool,
}

impl PerformanceProfile {
    pub fn from_low_end_pc(enabled: bool) -> Self {
        if enabled {
            Self {
                load_radius: 2,
                render_radius: 2,
                cleanup_radius: 3,
                fog_density_multiplier: 1.45,
                mesh_build_budget: 1,
                water_sim_interval: 0.55,
                water_rebuild_interval: 2.5,
                prefer_vsync: false,
                hide_dense_foliage: true,
            }
        } else {
            Self {
                load_radius: 4,
                render_radius: 4,
                cleanup_radius: 5,
                fog_density_multiplier: 1.0,
                mesh_build_budget: 2,
                water_sim_interval: 0.3,
                water_rebuild_interval: 1.5,
                prefer_vsync: true,
                hide_dense_foliage: false,
            }
        }
    }
}
