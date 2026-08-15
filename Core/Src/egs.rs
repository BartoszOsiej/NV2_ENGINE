//! EGS / Epic Online Services integration for NV-2.0.
//!
//! Strategy: the EOS SDK is a C library that must be downloaded from the
//! Epic Dev Portal (requires an account + a registered product). To keep the
//! game buildable and shippable **without** that SDK, we load it **at
//! runtime** via `libloading`. Consequences:
//!
//! * No build-time dependency on EOS headers or libs — `cargo build` works
//!   on any machine, with or without the SDK.
//! * If the SDK DLL/shared-object is missing, or the game was not launched
//!   by the Epic Games Launcher, every call degrades to a clean no-op and
//!   the game runs exactly as before (keyless EGS / itch.io / direct).
//! * When EGL launches the game (`-EpicPortal` / `-EpicApp`), we read the
//!   sandbox/deployment/product IDs and the ExchangeCode from the command
//!   line, initialise the platform, log the player in and report
//!   achievements/stats.
//!
//! What to fill in before release (one-time, requires a Dev Portal account):
//!   * Product ID, Sandbox ID, Deployment ID, Client ID (+ secret for dev
//!     builds) — either as `NV2_EOS_*` environment variables or in a
//!     `egs_config.json` next to the executable.
//!   * Drop `EOSSDK-Win64-Shipping.dll` (Windows) /
//!     `libEOSSDK-Linux-Shipping.so` (Linux) next to the game binary.
//! See `EGS/README.md` → "EOS SDK" for the step-by-step.

use std::ffi::{CStr, CString};
use std::path::Path;

// ─────────────────────────────────────────────────────────────── config ──

/// EOS platform credentials. All fields are `Option` so a config that is
/// only partially filled in still works (missing pieces → no EOS init,
/// game continues keyless).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EgsConfig {
    pub product_id: Option<String>,
    pub sandbox_id: Option<String>,
    pub deployment_id: Option<String>,
    pub client_id: Option<String>,
    /// Only needed for dev builds; player builds use the launcher's
    /// exchange code and never ship a secret.
    pub client_secret: Option<String>,
}

impl EgsConfig {
    /// Load from environment variables (`NV2_EOS_*`) and, if present, an
    /// `egs_config.json` next to the executable. Env vars win over the file.
    pub fn load() -> Self {
        let from_file = Self::from_json_file(Path::new("egs_config.json")).unwrap_or_default();
        Self {
            product_id: env_or("NV2_EOS_PRODUCT_ID", from_file.product_id),
            sandbox_id: env_or("NV2_EOS_SANDBOX_ID", from_file.sandbox_id),
            deployment_id: env_or("NV2_EOS_DEPLOYMENT_ID", from_file.deployment_id),
            client_id: env_or("NV2_EOS_CLIENT_ID", from_file.client_id),
            client_secret: env_or("NV2_EOS_CLIENT_SECRET", from_file.client_secret),
        }
    }

    fn from_json_file(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// True when every value needed to create an EOS platform is present.
    pub fn complete(&self) -> bool {
        self.product_id.as_deref().is_some_and(|s| !s.is_empty())
            && self.sandbox_id.as_deref().is_some_and(|s| !s.is_empty())
            && self.deployment_id.as_deref().is_some_and(|s| !s.is_empty())
            && self.client_id.as_deref().is_some_and(|s| !s.is_empty())
    }
}

fn env_or(name: &str, fallback: Option<String>) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty()).or(fallback)
}

/// What the Epic Games Launcher passes to a launched game. NV-2.0 reads the
/// subset it needs; everything else is ignored.
#[derive(Clone, Debug, Default)]
pub struct EpicLaunchArgs {
    pub launched_by_epic: bool,
    pub app_name: Option<String>,
    pub sandbox_id: Option<String>,
    pub deployment_id: Option<String>,
    pub product_id: Option<String>,
    /// `-AUTH_PASSWORD` — the ExchangeCode EGL gives us to prove ownership.
    pub exchange_code: Option<String>,
    /// `-AUTH_TYPE` — should be `exchangecode` when EGL provides one.
    pub auth_type: Option<String>,
}

impl EpicLaunchArgs {
    /// Parse the process argument list for EGL's launch arguments.
    pub fn from_args(args: &[String]) -> Self {
        let mut out = Self::default();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            let (key, inline_val) = match a.split_once('=') {
                Some((k, v)) => (k.to_ascii_lowercase(), Some(v.to_string())),
                None => (a.to_ascii_lowercase(), None),
            };
            let value = |i: &mut usize| -> Option<String> {
                if let Some(v) = inline_val.clone() {
                    return Some(v);
                }
                let next = args.get(*i + 1);
                *i += usize::from(next.is_some());
                next.cloned()
            };
            match key.as_str() {
                "-epicportal" | "-epicgames" => out.launched_by_epic = true,
                "-epicapp" | "-epicappid" => out.app_name = value(&mut i),
                "-epicsandboxid" | "-epicsandbox" => out.sandbox_id = value(&mut i),
                "-epicdeploymentid" | "-epicdeployment" => out.deployment_id = value(&mut i),
                "-epicproductid" | "-epicproduct" => out.product_id = value(&mut i),
                "-auth_password" | "-authpassword" => out.exchange_code = value(&mut i),
                "-auth_type" | "-authtype" => out.auth_type = value(&mut i),
                _ => {}
            }
            i += 1;
        }
        out
    }

    /// True when this looks like a real EGL launch with ownership proof.
    pub fn has_exchange_code(&self) -> bool {
        self.exchange_code.as_deref().is_some_and(|s| !s.is_empty())
    }
}

// ─────────────────────────────────────────────────────────────── FFI ─────

// Minimal hand-written bindings for the EOS functions NV-2.0 uses. The full
// SDK is huge; we only declare the entry points we call. Types follow the
// EOS SDK C API (1.17.x). These are loaded dynamically, so the structs are
// only ever used when the real SDK DLL is present next to the game.

/// Opaque platform handle (`EOS_HPlatform`).
pub type EosPlatformHandle = *mut std::ffi::c_void;
/// Opaque auth interface handle (`EOS_HAuth`).
pub type EosAuthHandle = *mut std::ffi::c_void;
/// Opaque achievements interface handle.
pub type EosAchievementsHandle = *mut std::ffi::c_void;
/// Opaque stats interface handle.
pub type EosStatsHandle = *mut std::ffi::c_void;
/// `EOS_EpicAccountId` (opaque, non-null = logged in).
pub type EosEpicAccountId = *mut std::ffi::c_void;

/// `EOS_EResult` — success = 0 (`EOS_Success`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EosResult {
    Success = 0,
    NoConnection = 1,
    InvalidAuth = 2,
    InvalidParameters = 3,
    NotFound = 4,
    Other = 5,
}

/// `EOS_ELoginStatus`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EosLoginStatus {
    NotLoggedIn = 0,
    UsingLocalProfile = 1,
    LoggedIn = 2,
}

/// `EOS_EAuthCredentialType` — we always use ExchangeCode on EGS.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EosAuthCredentialType {
    Password = 0,
    ExchangeCode = 1,
    PersistentAuth = 2,
    DeviceCode = 3,
    Developer = 4,
    RefreshToken = 5,
    AccountPortal = 6,
    ExternalAuth = 7,
}

// -- EOS_Platform_Options (layout follows the C SDK) ----------------------
// We keep the fields that matter for a game client. Pointers to C strings.

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EosPlatformClientCredentials {
    pub client_id: *const std::ffi::c_char,
    pub client_secret: *const std::ffi::c_char,
}

#[repr(C)]
pub struct EosPlatformOptions {
    pub api_version: i32,
    pub reserved: *mut std::ffi::c_void,
    pub product_id: *const std::ffi::c_char,
    pub sandbox_id: *const std::ffi::c_char,
    pub client_credentials: EosPlatformClientCredentials,
    pub is_server: u8,
    pub encryption_key: *const std::ffi::c_char,
    pub override_country_code: *const std::ffi::c_char,
    pub override_locale_code: *const std::ffi::c_char,
    pub deployment_id: *const std::ffi::c_char,
    pub flags: u64,
    pub cache_directory: *const std::ffi::c_char,
    pub tick_budget_in_milliseconds: u32,
    pub integrated_platform_options_container: EosPlatformHandle,
}

impl Default for EosPlatformOptions {
    fn default() -> Self {
        Self {
            // EOS_PLATFORM_OPTIONS_API_LATEST
            api_version: 12,
            reserved: std::ptr::null_mut(),
            product_id: std::ptr::null(),
            sandbox_id: std::ptr::null(),
            client_credentials: EosPlatformClientCredentials {
                client_id: std::ptr::null(),
                client_secret: std::ptr::null(),
            },
            is_server: 0,
            encryption_key: std::ptr::null(),
            override_country_code: std::ptr::null(),
            override_locale_code: std::ptr::null(),
            deployment_id: std::ptr::null(),
            flags: 0,
            cache_directory: std::ptr::null(),
            tick_budget_in_milliseconds: 0,
            integrated_platform_options_container: std::ptr::null_mut(),
        }
    }
}

// -- EOS_Auth_LoginOptions -------------------------------------------------

#[repr(C)]
pub struct EosAuthCredentials {
    pub api_version: i32,
    pub id: *const std::ffi::c_char,
    pub token: *const std::ffi::c_char,
    pub r#type: i32,
    pub system_auth_options: *mut std::ffi::c_void,
    pub external_type: i32,
}

#[repr(C)]
pub struct EosAuthLoginOptions {
    pub api_version: i32,
    pub credentials: *mut EosAuthCredentials,
    pub scope_flags: u64,
}

// -- EOS_Achievements_UnlockAchievementsOptions -----------------------------

#[repr(C)]
pub struct EosUnlockAchievementsOptions {
    pub api_version: i32,
    pub user_id: EosEpicAccountId,
    pub achievement_ids: *mut *const std::ffi::c_char,
    pub achievements_count: u32,
}

// -- EOS_Stats_IngestStat ----------------------------------------------------

#[repr(C)]
pub struct EosIngestStat {
    pub api_version: i32,
    pub stat_name: *const std::ffi::c_char,
    pub ingest_amount: i32,
}

#[repr(C)]
pub struct EosIngestStatOptions {
    pub api_version: i32,
    pub local_user_id: EosEpicAccountId,
    pub stats: *mut EosIngestStat,
    pub stats_count: u32,
}

type PlatformCreateFn = unsafe extern "C" fn(options: *const EosPlatformOptions) -> EosPlatformHandle;
type PlatformReleaseFn = unsafe extern "C" fn(handle: EosPlatformHandle);
type PlatformTickFn = unsafe extern "C" fn(handle: EosPlatformHandle);
type PlatformGetAuthFn = unsafe extern "C" fn(handle: EosPlatformHandle) -> EosAuthHandle;
type PlatformGetAchievementsFn =
    unsafe extern "C" fn(handle: EosPlatformHandle) -> EosAchievementsHandle;
type PlatformGetStatsFn = unsafe extern "C" fn(handle: EosPlatformHandle) -> EosStatsHandle;

type AuthLoginFn = unsafe extern "C" fn(
    handle: EosAuthHandle,
    options: *const EosAuthLoginOptions,
    client_data: *mut std::ffi::c_void,
    completion: extern "C" fn(*const EosAuthLoginCallbackInfo),
);
type AuthGetLoginStatusFn =
    unsafe extern "C" fn(handle: EosAuthHandle, account: EosEpicAccountId) -> i32;

type AchievementsUnlockFn = unsafe extern "C" fn(
    handle: EosAchievementsHandle,
    options: *const EosUnlockAchievementsOptions,
    client_data: *mut std::ffi::c_void,
    completion: extern "C" fn(*const EosUnlockCallbackInfo),
);

type StatsIngestStatFn = unsafe extern "C" fn(
    handle: EosStatsHandle,
    options: *const EosIngestStatOptions,
    client_data: *mut std::ffi::c_void,
    completion: extern "C" fn(*const EosIngestCallbackInfo),
);

#[repr(C)]
pub struct EosAuthLoginCallbackInfo {
    pub result_code: i32,
    pub client_data: *mut std::ffi::c_void,
    pub local_user_id: EosEpicAccountId,
    pub pin_grant_info: *mut std::ffi::c_void,
}

#[repr(C)]
pub struct EosUnlockCallbackInfo {
    pub result_code: i32,
    pub client_data: *mut std::ffi::c_void,
    pub user_id: EosEpicAccountId,
    pub achievements_count: u32,
    pub achievement_ids: *mut *const std::ffi::c_char,
}

#[repr(C)]
pub struct EosIngestCallbackInfo {
    pub result_code: i32,
    pub client_data: *mut std::ffi::c_void,
    pub local_user_id: EosEpicAccountId,
    pub stat_name: *const std::ffi::c_char,
}

// ─────────────────────────────────────────────────────────── runtime ─────

/// Result of a runtime operation, mapped to a friendly string for the HUD.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EosState {
    /// No SDK / not launched by Epic — nothing to do (keyless mode).
    Disabled,
    /// SDK loaded, config missing — waiting for Dev Portal credentials.
    WaitingForConfig,
    /// Platform created, login in progress.
    Connecting,
    /// Authenticated — achievements/stats reporting is live.
    Connected,
    /// Something failed (see log).
    Error,
}

impl EosState {
    pub fn label(self) -> &'static str {
        match self {
            EosState::Disabled => "EOS: disabled (keyless)",
            EosState::WaitingForConfig => "EOS: SDK found — add Dev Portal credentials",
            EosState::Connecting => "EOS: connecting…",
            EosState::Connected => "EOS: connected",
            EosState::Error => "EOS: error (see log)",
        }
    }
}

/// Live EOS bridge. Created once at startup; `new()` never fails even when
/// the SDK is absent — it just stays `Disabled`.
pub struct EosBridge {
    state: EosState,
    platform: EosPlatformHandle,
    auth: EosAuthHandle,
    achievements: EosAchievementsHandle,
    stats: EosStatsHandle,
    local_user: EosEpicAccountId,
    _library: Option<libloading::Library>,
    // Function pointers kept alive for the bridge's lifetime.
    platform_tick: Option<PlatformTickFn>,
    auth_get_login_status: Option<AuthGetLoginStatusFn>,
    achievements_unlock: Option<AchievementsUnlockFn>,
    stats_ingest: Option<StatsIngestStatFn>,
    platform_release: Option<PlatformReleaseFn>,
}

// Callbacks must be `extern "C"`. We keep the last auth result here so the
// main loop can read it without touching unsafe state across threads.
static AUTH_RESULT: std::sync::Mutex<Option<i32>> = std::sync::Mutex::new(None);

extern "C" fn on_auth_login(info: *const EosAuthLoginCallbackInfo) {
    let rc = unsafe { (*info).result_code };
    if let Ok(mut g) = AUTH_RESULT.lock() {
        *g = Some(rc);
    }
}

extern "C" fn on_unlock(_info: *const EosUnlockCallbackInfo) {}
extern "C" fn on_ingest(_info: *const EosIngestCallbackInfo) {}

impl EosBridge {
    /// Create the bridge. Safe to call always; returns a disabled bridge
    /// when the SDK is missing or the game was not launched by Epic.
    pub fn new(args: &EpicLaunchArgs) -> Self {
        let mut bridge = Self {
            state: EosState::Disabled,
            platform: std::ptr::null_mut(),
            auth: std::ptr::null_mut(),
            achievements: std::ptr::null_mut(),
            stats: std::ptr::null_mut(),
            local_user: std::ptr::null_mut(),
            _library: None,
            platform_tick: None,
            auth_get_login_status: None,
            achievements_unlock: None,
            stats_ingest: None,
            platform_release: None,
        };

        if !args.launched_by_epic {
            return bridge; // itch.io / direct launch — keyless as before.
        }

        // Find the SDK next to the executable (Windows DLL / Linux .so).
        let library = match load_sdk_library() {
            Some(lib) => lib,
            None => {
                log::info!("EOS SDK not found next to the executable — EGS keyless mode");
                return bridge;
            }
        };
        let symbols = unsafe {
            let platform_create: libloading::Symbol<PlatformCreateFn> =
                match library.get(b"EOS_Platform_Create") {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("EOS_Platform_Create not found: {e}");
                        return bridge;
                    }
                };
            let platform_release: libloading::Symbol<PlatformReleaseFn> =
                match library.get(b"EOS_Platform_Release") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            let platform_tick: libloading::Symbol<PlatformTickFn> =
                match library.get(b"EOS_Platform_Tick") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            let platform_get_auth: libloading::Symbol<PlatformGetAuthFn> =
                match library.get(b"EOS_Platform_GetAuthInterface") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            let platform_get_ach: libloading::Symbol<PlatformGetAchievementsFn> =
                match library.get(b"EOS_Platform_GetAchievementsInterface") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            let platform_get_stats: libloading::Symbol<PlatformGetStatsFn> =
                match library.get(b"EOS_Platform_GetStatsInterface") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            let auth_login: libloading::Symbol<AuthLoginFn> =
                match library.get(b"EOS_Auth_Login") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            let auth_login_status: libloading::Symbol<AuthGetLoginStatusFn> =
                match library.get(b"EOS_Auth_GetLoginStatus") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            let ach_unlock: libloading::Symbol<AchievementsUnlockFn> =
                match library.get(b"EOS_Achievements_UnlockAchievements") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            let stats_ingest: libloading::Symbol<StatsIngestStatFn> =
                match library.get(b"EOS_Stats_IngestStat") {
                    Ok(s) => s,
                    Err(_) => return bridge,
                };
            (
                *platform_create,
                *platform_release,
                *platform_tick,
                *platform_get_auth,
                *platform_get_ach,
                *platform_get_stats,
                *auth_login,
                *auth_login_status,
                *ach_unlock,
                *stats_ingest,
            )
        };

        let config = EgsConfig::load();
        if !config.complete() {
            log::info!("EOS SDK present but Dev Portal credentials missing — keyless");
            bridge._library = Some(library);
            bridge.platform_release = Some(symbols.1);
            bridge.state = EosState::WaitingForConfig;
            return bridge;
        }

        // Build the C strings we feed to the SDK.
        let (c_product, c_sandbox, c_deploy, c_client, c_secret) = (
            cstr(&config.product_id.unwrap_or_default()),
            cstr(&config.sandbox_id.unwrap_or_default()),
            cstr(&config.deployment_id.unwrap_or_default()),
            cstr(&config.client_id.unwrap_or_default()),
            cstr(&config.client_secret.unwrap_or_default()),
        );

        let mut options = EosPlatformOptions::default();
        options.product_id = c_product.as_ptr();
        options.sandbox_id = c_sandbox.as_ptr();
        options.deployment_id = c_deploy.as_ptr();
        options.client_credentials = EosPlatformClientCredentials {
            client_id: c_client.as_ptr(),
            client_secret: c_secret.as_ptr(),
        };

        let platform = unsafe { (symbols.0)(&options) };
        if platform.is_null() {
            log::error!("EOS_Platform_Create failed");
            bridge._library = Some(library);
            bridge.platform_release = Some(symbols.1);
            bridge.state = EosState::Error;
            return bridge;
        }

        let auth = unsafe { (symbols.3)(platform) };
        let achievements = unsafe { (symbols.4)(platform) };
        let stats = unsafe { (symbols.5)(platform) };

        bridge.platform = platform;
        bridge.auth = auth;
        bridge.achievements = achievements;
        bridge.stats = stats;
        bridge._library = Some(library);
        bridge.platform_tick = Some(symbols.2);
        bridge.auth_get_login_status = Some(symbols.7);
        bridge.achievements_unlock = Some(symbols.8);
        bridge.stats_ingest = Some(symbols.9);
        bridge.platform_release = Some(symbols.1);

        // Login with the exchange code from EGL, if present.
        if args.has_exchange_code() {
            let code = cstr(args.exchange_code.as_deref().unwrap_or(""));
            let mut creds = EosAuthCredentials {
                api_version: 3, // EOS_AUTH_CREDENTIALS_API_LATEST
                id: std::ptr::null(),
                token: code.as_ptr(),
                r#type: EosAuthCredentialType::ExchangeCode as i32,
                system_auth_options: std::ptr::null_mut(),
                external_type: 0,
            };
            let mut login_opts = EosAuthLoginOptions {
                api_version: 6, // EOS_AUTH_LOGIN_API_LATEST
                credentials: &mut creds,
                scope_flags: 0,
            };
            let _ = unsafe {
                (symbols.6)(
                    auth,
                    &login_opts,
                    std::ptr::null_mut(),
                    on_auth_login,
                )
            };
            bridge.state = EosState::Connecting;
        } else {
            bridge.state = EosState::Connected;
        }

        bridge
    }

    /// Poll the SDK and refresh the auth state. Call every frame.
    pub fn tick(&mut self, dt: f32) {
        if let Some(tick) = self.platform_tick {
            let handle = self.platform;
            // EOS_Platform_Tick is throttled internally; call ~per frame is fine.
            unsafe { (tick)(handle) };
        }
        if self.state == EosState::Connecting {
            if let Some(status) = self.auth_get_login_status {
                let status = unsafe { (status)(self.auth, self.local_user) };
                match status {
                    s if s == EosLoginStatus::LoggedIn as i32 => {
                        self.state = EosState::Connected;
                        log::info!("EOS: authenticated");
                    }
                    _ => {}
                }
            }
            // If the callback reported a hard failure, surface it.
            if let Ok(mut g) = AUTH_RESULT.lock() {
                if let Some(rc) = g.take() {
                    if rc != EosResult::Success as i32 {
                        self.state = EosState::Error;
                        log::error!("EOS_Auth_Login failed: result {rc}");
                    }
                }
            }
        }
        let _ = dt;
    }

    /// Report an unlocked achievement to EOS (no-op when not connected).
    pub fn unlock_achievement(&mut self, achievement_id: &str) {
        if self.state != EosState::Connected {
            return;
        }
        let id = cstr(achievement_id);
        let mut ids: Vec<*const std::ffi::c_char> = vec![id.as_ptr()];
        let mut opts = EosUnlockAchievementsOptions {
            api_version: 1, // EOS_ACHIEVEMENTS_UNLOCKACHIEVEMENTS_API_LATEST
            user_id: self.local_user,
            achievement_ids: ids.as_mut_ptr(),
            achievements_count: 1,
        };
        if let Some(unlock) = self.achievements_unlock {
            unsafe {
                (unlock)(
                    self.achievements,
                    &opts,
                    std::ptr::null_mut(),
                    on_unlock,
                )
            };
        }
    }

    /// Ingest a numeric stat to EOS (no-op when not connected).
    pub fn ingest_stat(&mut self, name: &str, amount: i32) {
        if self.state != EosState::Connected {
            return;
        }
        let stat_name = cstr(name);
        let mut stat = EosIngestStat {
            api_version: 3, // EOS_STATS_INGESTSTAT_API_LATEST
            stat_name: stat_name.as_ptr(),
            ingest_amount: amount,
        };
        let mut opts = EosIngestStatOptions {
            api_version: 3, // EOS_STATS_INGESTSTATOPTIONS_API_LATEST
            local_user_id: self.local_user,
            stats: &mut stat,
            stats_count: 1,
        };
        if let Some(ingest) = self.stats_ingest {
            unsafe {
                (ingest)(self.stats, &opts, std::ptr::null_mut(), on_ingest)
            };
        }
    }

    pub fn state(&self) -> EosState {
        self.state
    }
}

impl Drop for EosBridge {
    fn drop(&mut self) {
        if let Some(release) = self.platform_release {
            if !self.platform.is_null() {
                unsafe { (release)(self.platform) };
            }
        }
    }
}

// ────────────────────────────────────────────────────────────── helpers ──

/// Try to load the EOS SDK from next to the executable (or PATH).
fn load_sdk_library() -> Option<libloading::Library> {
    #[cfg(target_os = "windows")]
    let names: &[&str] = &["EOSSDK-Win64-Shipping.dll", "EOSSDK-Win64-Shipping.dll"];
    #[cfg(not(target_os = "windows"))]
    let names: &[&str] = &[
        "libEOSSDK-Linux-Shipping.so",
        "libEOSSDK-Linux-Shipping.a",
        "libEOSSDK.so",
    ];
    for name in names {
        if let Ok(lib) = unsafe { libloading::Library::new(name) } {
            log::info!("EOS SDK loaded: {name}");
            return Some(lib);
        }
    }
    None
}

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

// ──────────────────────────────────────────────────────────────── tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epic_args_parsing() {
        let args: Vec<String> = vec![
            "nv2_engine.exe".into(),
            "-EpicPortal".into(),
            "-EpicApp=NV20".into(),
            "-EpicEnv=Prod".into(),
            "-EpicLocale=en-US".into(),
            "-AUTH_LOGIN=unused".into(),
            "-AUTH_PASSWORD=exchange_code_abc".into(),
            "-AUTH_TYPE=exchangecode".into(),
            "-epicsandboxid".into(),
            "sandbox123".into(),
        ];
        let parsed = EpicLaunchArgs::from_args(&args);
        assert!(parsed.launched_by_epic);
        assert_eq!(parsed.app_name.as_deref(), Some("NV20"));
        assert_eq!(parsed.exchange_code.as_deref(), Some("exchange_code_abc"));
        assert_eq!(parsed.auth_type.as_deref(), Some("exchangecode"));
        assert_eq!(parsed.sandbox_id.as_deref(), Some("sandbox123"));
        assert!(parsed.has_exchange_code());
    }

    #[test]
    fn epic_args_absent_when_not_launched_by_egl() {
        let parsed = EpicLaunchArgs::from_args(&["nv2_engine.exe".to_string()]);
        assert!(!parsed.launched_by_epic);
        assert!(!parsed.has_exchange_code());
    }

    #[test]
    fn config_complete_requires_all_ids() {
        let mut c = EgsConfig::default();
        assert!(!c.complete());
        c.product_id = Some("p".into());
        c.sandbox_id = Some("s".into());
        c.deployment_id = Some("d".into());
        assert!(!c.complete());
        c.client_id = Some("c".into());
        assert!(c.complete());
    }

    #[test]
    fn disabled_bridge_without_epic_launch() {
        let bridge = EosBridge::new(&EpicLaunchArgs::default());
        assert_eq!(bridge.state(), EosState::Disabled);
    }

    #[test]
    fn label_strings_exist() {
        for s in [
            EosState::Disabled,
            EosState::WaitingForConfig,
            EosState::Connecting,
            EosState::Connected,
            EosState::Error,
        ] {
            assert!(!s.label().is_empty());
        }
    }
}
