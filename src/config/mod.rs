use std::env;
use std::path::Path;

use once_cell::sync::OnceCell;
use serde::Deserialize;

#[doc(hidden)]
static SETTINGS: OnceCell<Settings> = OnceCell::new();

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Settings {
    pub brokers: String,
    pub grpc_addr: String,
    pub db: String,
    pub persist_dir: Box<Path>,
    pub persist_every_n_block: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    /// Initializes with parsing config file in env var `CONFIG`.
    ///
    /// # panics
    /// if the `CONFIG` env var not exist or the file is corrupt, it panics.
    pub fn init_default() {
        // Initializes with `config/default.yaml`.
        let mut conf = config_rs::Config::default();
        conf.merge(config_rs::File::with_name("config/default")).unwrap();

        // Merges with `config/RUN_MODE.yaml` (development as default).
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        conf.merge(config_rs::File::with_name(&format!("config/{}", run_mode)).required(false))
            .unwrap();

        Self::set(conf.try_into().unwrap());
    }

    pub fn new() -> Self {
        Settings {
            brokers: String::new(),
            grpc_addr: String::new(),
            db: String::new(),
            persist_dir: Box::from(Path::new(".")),
            persist_every_n_block: 0,
        }
    }

    /// Sets the contents of this cell to the singleton `Settings`
    /// and returns the reference to it.
    ///
    /// # Panics
    /// if the underlying cell is full, it panics.
    pub fn set(settings: Self) -> &'static Self {
        SETTINGS.set(settings).unwrap();
        Self::get()
    }

    /// Gets the reference to the singleton `Settings`.
    ///
    /// # Panics
    /// if the underlying cell is empty, it panics.
    pub fn get() -> &'static Self {
        SETTINGS.get().unwrap()
    }

    /// Shortcut of `Self::get().brokers.as_str()`
    #[inline(always)]
    pub fn brokers() -> &'static str {
        Self::get().brokers.as_str()
    }

    /// Shortcut of `Self::get().grpc_addr.as_str()`
    #[inline(always)]
    pub fn grpc_addr() -> &'static str {
        Self::get().grpc_addr.as_str()
    }

    /// Shortcut of `Self::get().rollup_state_manager_db.as_str()`
    #[inline(always)]
    pub fn db() -> &'static str {
        Self::get().db.as_str()
    }

    pub fn persist_dir() -> &'static Path {
        Self::get().persist_dir.as_ref()
    }

    /// Shortcut of `Self::get().persist_every_n_block`
    #[inline(always)]
    pub fn persist_every_n_block() -> usize {
        Self::get().persist_every_n_block
    }
}
