use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const LAUNCHER_VERSION: &str = "1.0.0";
#[allow(dead_code)]
pub const SOURCE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_SERVER_URL: &str = "https://aaa-mmorpg-engine-danielbodnar2.replit.app";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub install_dir: PathBuf,
    pub o3de_version: String,
    pub vulkan_version: String,
    pub tracy_version: String,
    pub force_rebuild: bool,
    pub skip_update: bool,
    pub verbose: bool,
}

impl Default for Config {
    fn default() -> Self {
        let install_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\"))
            .join("AAAEngine");

        Self {
            server_url: DEFAULT_SERVER_URL.to_string(),
            install_dir,
            o3de_version: "2510.1".to_string(),  // GitHub tag format (25.10.1 -> 2510.1)
            vulkan_version: "1.3.290.0".to_string(),
            tracy_version: "0.11.1".to_string(),
            force_rebuild: false,
            skip_update: false,
            verbose: false,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        
        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)?
        } else {
            Config::default()
        };
        
        // Override server URL from server_url.txt if it exists (written by bootstrap.bat)
        // This ensures we use the correct dev server URL for the current session
        let server_url_file = config.install_dir.join("server_url.txt");
        if let Ok(url) = std::fs::read_to_string(&server_url_file) {
            let url = url.trim();
            if !url.is_empty() {
                config.server_url = url.to_string();
            }
        }
        
        // Also check environment variable
        if let Ok(url) = std::env::var("AAA_SERVER_URL") {
            if !url.is_empty() {
                config.server_url = url;
            }
        }
        
        config.save()?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();
        
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\"))
            .join("AAAEngine")
            .join("launcher_config.json")
    }

    pub fn deps_dir(&self) -> PathBuf {
        self.install_dir.join("deps")
    }

    pub fn o3de_dir(&self) -> PathBuf {
        self.install_dir.join("o3de")
    }

    pub fn engine_dir(&self) -> PathBuf {
        self.install_dir.join("engine")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.install_dir.join("logs")
    }

    pub fn vulkan_sdk_dir(&self) -> PathBuf {
        self.deps_dir().join(format!("VulkanSDK\\{}", self.vulkan_version))
    }

    pub fn tracy_dir(&self) -> PathBuf {
        self.deps_dir().join(format!("tracy-{}", self.tracy_version))
    }
}

#[allow(dead_code)]
pub fn dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("com", "AAAStudio", "AAAEngine")
}
