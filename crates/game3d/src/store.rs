//! Small records kept between visits: the best score and the sound setting. In the browser they live in
//! localStorage; natively, one file per key in a directory (the temp directory unless a test points it elsewhere).
//! The keys and formats are the 2.5D game's (`slayback-best`, `slayback-sound`), so a best carries over between them.

use bevy::prelude::*;

/// Where records are kept.
#[derive(Resource, Clone, Debug, Default)]
pub struct Store {
    /// Natively, the directory (the temp directory if none).
    #[cfg(not(target_arch = "wasm32"))]
    dir: Option<std::path::PathBuf>,
}

impl Store {
    /// Natively, records kept as files in `dir` (headless scenarios use their own, so they never touch a real best).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn in_dir(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: Some(dir.into()) }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn file(&self, key: &str) -> std::path::PathBuf {
        self.dir.clone().unwrap_or_else(std::env::temp_dir).join(key)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        return storage()?.get_item(key).ok()?;
        #[cfg(not(target_arch = "wasm32"))]
        std::fs::read_to_string(self.file(key)).ok()
    }

    /// Keeps `value` under `key`. Failures (private browsing, blocked storage) are ignored: a lost record only costs
    /// the best line or the sound setting.
    pub fn set(&self, key: &str, value: &str) {
        #[cfg(target_arch = "wasm32")]
        if let Some(s) = storage() {
            let _ = s.set_item(key, value);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = std::fs::write(self.file(key), value);
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}
