use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::config::Config;
use crate::logging;

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    #[allow(dead_code)]
    pub version: String,
    pub checksum: String,
}

#[derive(Debug, serde::Deserialize)]
struct VersionResponse {
    version: String,
    checksum: Option<String>,
}

pub struct Updater {
    config: Config,
    client: reqwest::Client,
}

impl Updater {
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self { config, client })
    }

    pub async fn check_for_update(&self) -> Result<Option<UpdateInfo>> {
        let url = format!("{}/sync/launcher-version", self.config.server_url);
        
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to update server")?;

        if !response.status().is_success() {
            logging::warn("Could not check for updates - server unavailable");
            return Ok(None);
        }

        let version_info: VersionResponse = response
            .json()
            .await
            .context("Failed to parse version response")?;

        let current_version = crate::config::LAUNCHER_VERSION;
        
        if version_info.version != current_version {
            let checksum = version_info.checksum.ok_or_else(|| {
                anyhow::anyhow!("Server did not provide checksum for update - refusing to update")
            })?;
            
            logging::info(&format!(
                "Update available: {} -> {}",
                current_version, version_info.version
            ));
            
            Ok(Some(UpdateInfo {
                version: version_info.version,
                checksum,
            }))
        } else {
            logging::success("Launcher is up to date");
            Ok(None)
        }
    }

    pub async fn download_update(&self, temp_path: &Path) -> Result<()> {
        let url = format!("{}/sync/launcher-binary", self.config.server_url);
        
        logging::download("Downloading launcher update...");
        
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to download update")?;

        if !response.status().is_success() {
            anyhow::bail!("Update download failed: HTTP {}", response.status());
        }

        let total_size = response.content_length().unwrap_or(0);
        let pb = logging::progress_bar(total_size);

        let bytes = response.bytes().await?;
        pb.finish_and_clear();

        std::fs::write(temp_path, &bytes).context("Failed to write update file")?;

        logging::success("Update downloaded");
        Ok(())
    }

    pub fn verify_checksum(file_path: &Path, expected: &str) -> Result<bool> {
        let bytes = std::fs::read(file_path)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let result = hex::encode(hasher.finalize());
        
        Ok(result == expected)
    }

    pub async fn download_and_verify(&self, temp_path: &Path, expected_checksum: &str) -> Result<()> {
        self.download_update(temp_path).await?;
        
        logging::info("Verifying update checksum...");
        
        if !Self::verify_checksum(temp_path, expected_checksum)? {
            if temp_path.exists() {
                let _ = std::fs::remove_file(temp_path);
            }
            anyhow::bail!(
                "Checksum verification failed! Update file may be corrupted or tampered with. Expected: {}",
                expected_checksum
            );
        }
        
        logging::success("Checksum verified");
        Ok(())
    }

    pub fn apply_update(temp_path: &Path, target_path: &Path) -> Result<()> {
        let backup_path = target_path.with_extension("old");
        
        if target_path.exists() {
            std::fs::rename(target_path, &backup_path)
                .context("Failed to backup current launcher")?;
        }

        match std::fs::rename(temp_path, target_path) {
            Ok(_) => {
                if backup_path.exists() {
                    let _ = std::fs::remove_file(&backup_path);
                }
                logging::success("Update applied successfully");
                Ok(())
            }
            Err(e) => {
                if backup_path.exists() {
                    let _ = std::fs::rename(&backup_path, target_path);
                }
                Err(e).context("Failed to apply update")
            }
        }
    }

    pub fn request_restart() -> ! {
        logging::info("Launcher updated - please restart");
        std::process::exit(0);
    }
}
