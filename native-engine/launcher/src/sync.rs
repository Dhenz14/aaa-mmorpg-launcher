use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::logging;

#[derive(Debug, serde::Deserialize)]
pub struct FileManifest {
    #[allow(dead_code)]
    pub version: String,
    pub files: HashMap<String, FileInfo>,
}

#[derive(Debug, serde::Deserialize)]
pub struct FileInfo {
    pub checksum: String,
    pub size: u64,
}

#[derive(Debug, serde::Deserialize)]
struct VersionResponse {
    version: String,
}

pub struct SyncManager {
    config: Config,
    client: reqwest::Client,
}

impl SyncManager {
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(600))  // 10 minutes for large downloads
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self { config, client })
    }

    pub async fn check_server(&self) -> Result<String> {
        let url = format!("{}/sync/version", self.config.server_url);
        
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to server")?;

        if !response.status().is_success() {
            anyhow::bail!("Server returned error: {}", response.status());
        }

        let version_info: VersionResponse = response
            .json()
            .await
            .context("Failed to parse server version")?;

        logging::success(&format!("Connected to server v{}", version_info.version));
        Ok(version_info.version)
    }

    pub async fn get_manifest(&self) -> Result<FileManifest> {
        let url = format!("{}/sync/manifest", self.config.server_url);
        
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch manifest")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get manifest: {}", response.status());
        }

        let manifest: FileManifest = response
            .json()
            .await
            .context("Failed to parse manifest")?;

        Ok(manifest)
    }

    pub fn calculate_checksum(path: &Path) -> Result<String> {
        let bytes = std::fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(hex::encode(hasher.finalize()))
    }

    pub async fn sync_files(&self, manifest: &FileManifest) -> Result<u64> {
        let engine_dir = self.config.engine_dir();
        std::fs::create_dir_all(&engine_dir)?;

        let mut synced_count = 0u64;

        for (file_path, info) in &manifest.files {
            let native_path = Self::normalize_path_for_platform(file_path);
            let local_path = engine_dir.join(&native_path);
            let needs_sync = self.file_needs_sync(&local_path, info)?;

            if needs_sync {
                self.download_file(file_path, &local_path, info).await?;
                synced_count += 1;
            }
        }

        if synced_count > 0 {
            logging::success(&format!("Synced {} files", synced_count));
        } else {
            logging::success("All files up to date");
        }

        Ok(synced_count)
    }

    fn normalize_path_for_platform(path: &str) -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(path.replace('/', "\\"))
        }
        #[cfg(not(windows))]
        {
            PathBuf::from(path)
        }
    }

    fn file_needs_sync(&self, local_path: &Path, info: &FileInfo) -> Result<bool> {
        if !local_path.exists() {
            return Ok(true);
        }

        let metadata = std::fs::metadata(local_path)?;
        if metadata.len() != info.size {
            return Ok(true);
        }

        let local_checksum = Self::calculate_checksum(local_path)?;
        Ok(local_checksum != info.checksum)
    }

    async fn download_file(
        &self,
        remote_path: &str,
        local_path: &Path,
        info: &FileInfo,
    ) -> Result<()> {
        let url = format!("{}/sync/file/{}", self.config.server_url, remote_path);

        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        logging::download(&format!("Downloading {}", remote_path));

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Download request failed")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download {}: {}", remote_path, response.status());
        }

        let bytes = response.bytes().await?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let checksum = hex::encode(hasher.finalize());

        if checksum != info.checksum {
            anyhow::bail!(
                "Checksum mismatch for {}: expected {}, got {}",
                remote_path,
                info.checksum,
                checksum
            );
        }

        std::fs::write(local_path, &bytes)?;
        Ok(())
    }

    pub async fn download_full_archive(&self) -> Result<()> {
        let url = format!("{}/sync/full.zip", self.config.server_url);
        let archive_path = self.config.install_dir.join("engine.zip");
        let engine_dir = self.config.engine_dir();

        // ALWAYS clear the engine cache before downloading fresh code
        // This ensures we never run stale/outdated builds
        if engine_dir.exists() {
            logging::info("Clearing cached engine files...");
            if let Err(e) = std::fs::remove_dir_all(&engine_dir) {
                logging::warn(&format!("Could not clear cache: {} - continuing anyway", e));
            } else {
                logging::success("Cache cleared");
            }
        }

        logging::info("Downloading full engine archive...");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to download archive")?;

        if !response.status().is_success() {
            anyhow::bail!("Archive download failed: {}", response.status());
        }

        let total_size = response.content_length().unwrap_or(0);
        let pb = logging::progress_bar(total_size);

        let bytes = response.bytes().await?;
        pb.finish_and_clear();

        std::fs::write(&archive_path, &bytes)?;

        logging::info("Extracting archive...");
        let file = std::fs::File::open(&archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(self.config.engine_dir())?;

        std::fs::remove_file(&archive_path)?;

        logging::success("Engine files extracted");
        Ok(())
    }
}
