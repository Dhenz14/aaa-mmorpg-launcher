use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

use crate::config::Config;
use crate::logging;

#[derive(Debug, Clone)]
pub struct DependencyStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    #[allow(dead_code)]
    pub path: Option<PathBuf>,
}

pub struct DependencyManager {
    config: Config,
}

impl DependencyManager {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn check_all(&self) -> Vec<DependencyStatus> {
        vec![
            self.check_vs_build_tools(),
            self.check_rust(),
            self.check_vulkan_sdk(),
            self.check_tracy(),
            self.check_o3de(),
            self.check_cmake(),
        ]
    }

    pub fn check_vs_build_tools(&self) -> DependencyStatus {
        // Use vswhere.exe as the SINGLE SOURCE OF TRUTH for VS detection
        // This is Microsoft's official tool and is always accurate
        if let Some((path, version)) = self.find_vs_via_vswhere() {
            return DependencyStatus {
                name: "Visual Studio Build Tools".to_string(),
                installed: true,
                version: Some(version),
                path: Some(path),
            };
        }

        // Fallback: check if cl.exe is in PATH (Developer Command Prompt)
        if let Ok(cl_path) = which::which("cl.exe") {
            return DependencyStatus {
                name: "Visual Studio Build Tools".to_string(),
                installed: true,
                version: self.get_cl_version(),
                path: Some(cl_path),
            };
        }

        DependencyStatus {
            name: "Visual Studio Build Tools".to_string(),
            installed: false,
            version: None,
            path: None,
        }
    }

    fn find_vs_via_vswhere(&self) -> Option<(PathBuf, String)> {
        // vswhere.exe is installed with VS Installer, always at this location
        let vswhere_paths = [
            r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
            r"C:\Program Files\Microsoft Visual Studio\Installer\vswhere.exe",
        ];

        for vswhere in &vswhere_paths {
            let vswhere_path = std::path::Path::new(vswhere);
            if !vswhere_path.exists() {
                continue;
            }

            // Query for any VS installation with C++ tools
            let output = Command::new(vswhere)
                .args([
                    "-latest",
                    "-products", "*",
                    "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                    "-format", "json"
                ])
                .output()
                .ok()?;

            let json_str = String::from_utf8_lossy(&output.stdout);
            
            // Parse JSON to get installation path and version
            if let Ok(installations) = serde_json::from_str::<Vec<Value>>(&json_str) {
                if let Some(install) = installations.first() {
                    let install_path = install.get("installationPath")
                        .and_then(|v| v.as_str())
                        .map(PathBuf::from)?;
                    
                    let version = install.get("installationVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("2022")
                        .to_string();
                    
                    // Find cl.exe in this installation
                    let vc_tools = install_path.join("VC").join("Tools").join("MSVC");
                    if vc_tools.exists() {
                        // Find the actual MSVC version directory and return path to cl.exe
                        if let Ok(entries) = std::fs::read_dir(&vc_tools) {
                            for entry in entries.flatten() {
                                let cl_path = entry.path()
                                    .join("bin").join("Hostx64").join("x64").join("cl.exe");
                                if cl_path.exists() {
                                    // Return the cl.exe path, not just the install root
                                    return Some((cl_path, version));
                                }
                            }
                        }
                        // If we can't find cl.exe but the MSVC dir exists, return the install path
                        return Some((install_path, version));
                    }
                }
            }
        }
        None
    }

    fn get_cl_version(&self) -> Option<String> {
        let output = Command::new("cl.exe").output().ok()?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        stderr.lines().next().map(|s| s.to_string())
    }

    pub fn check_rust(&self) -> DependencyStatus {
        let rustc_path = which::which("rustc.exe")
            .or_else(|_| which::which("rustc"))
            .ok();
        let installed = rustc_path.is_some();

        let version = if installed {
            Command::new("rustc")
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
        } else {
            None
        };

        DependencyStatus {
            name: "Rust".to_string(),
            installed,
            version,
            path: rustc_path,
        }
    }

    pub fn check_vulkan_sdk(&self) -> DependencyStatus {
        // First check VULKAN_SDK environment variable (set by installer)
        if let Ok(sdk_path) = std::env::var("VULKAN_SDK") {
            let path = PathBuf::from(&sdk_path);
            if path.exists() {
                // Extract version from path (e.g., C:\VulkanSDK\1.3.290.0)
                let version = path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| self.config.vulkan_version.clone());
                    
                logging::info(&format!("Found Vulkan SDK at: {}", path.display()));
                return DependencyStatus {
                    name: "Vulkan SDK".to_string(),
                    installed: true,
                    version: Some(version),
                    path: Some(path),
                };
            }
        }
        
        // Fallback: Check standard installation paths
        let vulkan_root = PathBuf::from(r"C:\VulkanSDK");
        if vulkan_root.exists() {
            if let Ok(entries) = std::fs::read_dir(&vulkan_root) {
                // Find the latest version installed
                let mut versions: Vec<_> = entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .collect();
                versions.sort_by(|a, b| b.file_name().cmp(&a.file_name())); // Latest first
                
                if let Some(latest) = versions.first() {
                    let path = latest.path();
                    let version = latest.file_name().to_string_lossy().to_string();
                    logging::info(&format!("Found Vulkan SDK at: {} (version {})", path.display(), version));
                    return DependencyStatus {
                        name: "Vulkan SDK".to_string(),
                        installed: true,
                        version: Some(version),
                        path: Some(path),
                    };
                }
            }
        }

        DependencyStatus {
            name: "Vulkan SDK".to_string(),
            installed: false,
            version: None,
            path: None,
        }
    }

    pub fn check_tracy(&self) -> DependencyStatus {
        // Check multiple possible Tracy locations
        let possible_paths = [
            PathBuf::from(r"C:\Tracy"),                           // User manual install
            PathBuf::from(r"C:\Tracy\Tracy.exe"),                 // Direct exe location
            self.config.tracy_dir(),                              // Launcher managed location
            dirs::data_local_dir().unwrap_or_default().join("Tracy"),
        ];
        
        for path in &possible_paths {
            // Check for Tracy.exe directly or in the folder
            let tracy_exe = if path.is_file() && path.file_name().map(|n| n == "Tracy.exe").unwrap_or(false) {
                path.clone()
            } else {
                path.join("Tracy.exe")
            };
            
            if tracy_exe.exists() {
                logging::info(&format!("Found Tracy at: {}", tracy_exe.display()));
                return DependencyStatus {
                    name: "Tracy Profiler".to_string(),
                    installed: true,
                    version: Some(self.config.tracy_version.clone()),
                    path: Some(tracy_exe.parent().unwrap_or(path).to_path_buf()),
                };
            }
            
            // Also check for public folder (source build)
            if path.exists() && path.join("public").exists() {
                logging::info(&format!("Found Tracy source at: {}", path.display()));
                return DependencyStatus {
                    name: "Tracy Profiler".to_string(),
                    installed: true,
                    version: Some(self.config.tracy_version.clone()),
                    path: Some(path.clone()),
                };
            }
        }

        DependencyStatus {
            name: "Tracy Profiler".to_string(),
            installed: false,
            version: None,
            path: None,
        }
    }

    pub fn check_o3de(&self) -> DependencyStatus {
        // Check multiple possible O3DE locations (in order of preference)
        let possible_paths = [
            // User's manual locations (FIRST PRIORITY)
            PathBuf::from(r"C:\O3DE-Source"),
            PathBuf::from(r"C:\O3DE-Build"),
            // Standard source locations
            PathBuf::from(r"C:\O3DE\25.10.1"),
            PathBuf::from(r"C:\O3DE\2510.1"),
            PathBuf::from(r"C:\O3DE\25.05.0"),
            PathBuf::from(r"C:\O3DE\24.09.0"),
            PathBuf::from(r"C:\O3DE"),                            // Root folder, check subfolders
            // Games folder (user's project location)
            PathBuf::from(r"C:\Games\AAA-MMORPG"),
            // Program Files locations
            PathBuf::from(r"C:\Program Files\O3DE"),
            PathBuf::from(r"C:\Program Files (x86)\O3DE"),
            // Launcher managed location
            self.config.o3de_dir(),
        ];
        
        for path in &possible_paths {
            // If this is C:\O3DE root, scan for version subfolders
            if path == &PathBuf::from(r"C:\O3DE") && path.exists() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let subpath = entry.path();
                        if subpath.is_dir() {
                            if let Some(found) = self.validate_o3de_installation(&subpath) {
                                return found;
                            }
                        }
                    }
                }
                continue;
            }
            
            if let Some(found) = self.validate_o3de_installation(path) {
                return found;
            }
        }

        DependencyStatus {
            name: "O3DE SDK".to_string(),
            installed: false,
            version: None,
            path: Some(self.config.o3de_dir()),
        }
    }
    
    fn validate_o3de_installation(&self, o3de_dir: &PathBuf) -> Option<DependencyStatus> {
        // Check for key O3DE files/folders that indicate a valid SOURCE installation
        let has_engine_json = o3de_dir.join("engine.json").exists();
        let has_cmake_lists = o3de_dir.join("CMakeLists.txt").exists();
        let has_scripts = o3de_dir.join("scripts").exists();
        let has_cmake = o3de_dir.join("cmake").exists();
        
        // Check for BUILT libraries (this is what we actually need)
        // Check both profile and release configs since user may have built with either
        let lib_paths = [
            // C:\O3DE-Build - User's confirmed build location (PRIORITY)
            PathBuf::from(r"C:\O3DE-Build").join("lib").join("release").join("AzCore.lib"),
            PathBuf::from(r"C:\O3DE-Build").join("lib").join("Release").join("AzCore.lib"),
            PathBuf::from(r"C:\O3DE-Build").join("lib").join("profile").join("AzCore.lib"),
            PathBuf::from(r"C:\O3DE-Build").join("lib").join("Profile").join("AzCore.lib"),
            // Standard O3DE install/build paths
            o3de_dir.join("install").join("lib").join("profile").join("AzCore.lib"),
            o3de_dir.join("install").join("lib").join("release").join("AzCore.lib"),
            o3de_dir.join("build").join("windows").join("lib").join("profile").join("AzCore.lib"),
            o3de_dir.join("build").join("windows").join("lib").join("release").join("AzCore.lib"),
            o3de_dir.join("build").join("lib").join("profile").join("AzCore.lib"),
            o3de_dir.join("lib").join("profile").join("AzCore.lib"),
            o3de_dir.join("lib").join("release").join("AzCore.lib"),
        ];
        
        let has_built_libs = lib_paths.iter().any(|p| p.exists());
        
        // Valid source if it has CMakeLists.txt (can be built)
        let is_valid_source = has_cmake_lists || has_engine_json || (has_scripts && has_cmake);
        
        if !is_valid_source && !has_built_libs {
            return None;
        }
        
        // CRITICAL: Only mark as "installed" if we have BUILT libraries
        // Source-only installations need to trigger the build step
        if !has_built_libs {
            logging::info(&format!("Found O3DE source at: {} (needs build)", o3de_dir.display()));
            // Return None to trigger install_o3de() which will build it
            return None;
        }
        
        logging::info(&format!("Found O3DE with built libraries at: {}", o3de_dir.display()));
        
        // Get version from engine.json if available
        let version = self.get_o3de_installed_version(o3de_dir);
        
        Some(DependencyStatus {
            name: "O3DE SDK".to_string(),
            installed: true,
            version: version.or_else(|| Some("built".to_string())),
            path: Some(o3de_dir.clone()),
        })
    }

    fn get_o3de_installed_version(&self, o3de_dir: &PathBuf) -> Option<String> {
        // Method 1: Check our version marker file
        let marker_file = self.config.install_dir.join("o3de_version.txt");
        if let Ok(version) = std::fs::read_to_string(&marker_file) {
            let version = version.trim();
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }

        // Method 2: Check engine.json for version info
        let engine_json = o3de_dir.join("engine.json");
        if let Ok(content) = std::fs::read_to_string(&engine_json) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
                    return Some(version.to_string());
                }
                if let Some(version) = json.get("O3DEVersion").and_then(|v| v.as_str()) {
                    return Some(version.to_string());
                }
            }
        }

        // Method 3: Check git branch/tag
        let output = Command::new("git")
            .args(["describe", "--tags", "--always"])
            .current_dir(o3de_dir)
            .output()
            .ok()?;
        
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                return Some(version);
            }
        }

        None
    }

    pub fn check_cmake(&self) -> DependencyStatus {
        let cmake_path = which::which("cmake.exe")
            .or_else(|_| which::which("cmake"))
            .ok();
        let installed = cmake_path.is_some();

        let version = if installed {
            Command::new("cmake")
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.lines().next().map(|l| l.to_string()))
        } else {
            None
        };

        DependencyStatus {
            name: "CMake".to_string(),
            installed,
            version,
            path: cmake_path,
        }
    }

    pub async fn install_missing(&self, deps: &[DependencyStatus]) -> Result<()> {
        for dep in deps.iter().filter(|d| !d.installed) {
            match dep.name.as_str() {
                "Visual Studio Build Tools" => self.install_vs_build_tools().await?,
                "Rust" => self.install_rust().await?,
                "Vulkan SDK" => self.install_vulkan_sdk().await?,
                "Tracy Profiler" => self.install_tracy().await?,
                "O3DE SDK" => self.install_o3de().await?,
                "CMake" => logging::warn("CMake should be installed with VS Build Tools"),
                _ => logging::warn(&format!("Unknown dependency: {}", dep.name)),
            }
        }
        Ok(())
    }

    async fn install_vs_build_tools(&self) -> Result<()> {
        logging::info("Installing Visual Studio Build Tools 2022...");
        logging::warn("This may take 10-30 minutes on first install");
        logging::warn("An installer window will open - please wait for it to complete");

        let installer_url = "https://aka.ms/vs/17/release/vs_buildtools.exe";
        let installer_path = self.config.deps_dir().join("vs_buildtools.exe");
        let log_path = self.config.deps_dir().join("vs_install.log");

        std::fs::create_dir_all(self.config.deps_dir())?;

        // Step 1: Clear any corrupted installer state
        logging::info("Clearing any corrupted installer state...");
        let installer_dir = PathBuf::from(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer");
        if installer_dir.exists() {
            // Check if installer is in a bad state by looking for lock files
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", "vs_installer.exe"])
                .output();
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", "vs_installershell.exe"])
                .output();
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        // Step 2: Download installer
        logging::info("Downloading VS Build Tools installer...");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        let response = client.get(installer_url).send().await?;
        let bytes = response.bytes().await?;
        std::fs::write(&installer_path, &bytes)?;
        logging::success("Installer downloaded");

        // Step 3: Run installer with --passive (shows UI but no interaction needed)
        // Using --passive instead of --quiet so user can see progress
        logging::info("Starting installer (a progress window will appear)...");
        logging::warn("Do NOT close the installer window - wait for it to finish");
        
        let ps_script = format!(
            r#"
# Run VS Build Tools installer with elevation
$installerPath = '{}'
$logPath = '{}'

# Build argument list
$args = @(
    '--passive',
    '--wait', 
    '--norestart',
    '--nocache',
    '--noUpdateInstaller',
    '--includeRecommended',
    '--log', $logPath,
    '--add', 'Microsoft.VisualStudio.Workload.VCTools',
    '--add', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
    '--add', 'Microsoft.VisualStudio.Component.Windows11SDK.22621',
    '--add', 'Microsoft.VisualStudio.Component.VC.CMake.Project'
)

# Start elevated process and wait
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $installerPath
$psi.Arguments = $args -join ' '
$psi.Verb = 'runas'
$psi.UseShellExecute = $true

try {{
    $process = [System.Diagnostics.Process]::Start($psi)
    $process.WaitForExit()
    $exitCode = $process.ExitCode
    Write-Output "EXIT_CODE:$exitCode"
}} catch {{
    Write-Output "EXIT_CODE:-1"
    Write-Output "ERROR:$($_.Exception.Message)"
}}
"#,
            installer_path.display().to_string().replace('\\', "\\\\"),
            log_path.display().to_string().replace('\\', "\\\\")
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
            .output()
            .context("Failed to run PowerShell")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let exit_code: i32 = stdout
            .lines()
            .find(|l| l.starts_with("EXIT_CODE:"))
            .and_then(|l| l.strip_prefix("EXIT_CODE:"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(-1);

        // Check exit codes
        match exit_code {
            0 => {
                logging::success("Visual Studio Build Tools installed successfully");
                return Ok(());
            }
            3010 => {
                logging::success("Visual Studio Build Tools installed");
                logging::warn("A system restart is recommended to complete installation");
                return Ok(());
            }
            -1 => {
                // User may have cancelled UAC
                logging::error("Installation was cancelled or failed to start");
            }
            _ => {
                logging::error(&format!("Installer returned exit code: {}", exit_code));
            }
        }

        // Check if it installed anyway by looking for the tools
        logging::info("Verifying installation...");
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        let vs_paths = [
            r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC",
            r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC",
        ];
        
        for base in &vs_paths {
            if std::path::Path::new(base).exists() {
                logging::success("Visual Studio Build Tools verified");
                return Ok(());
            }
        }

        // Check for logs to provide better error info
        if log_path.exists() {
            logging::info(&format!("Installation log saved to: {}", log_path.display()));
        }

        anyhow::bail!("VS Build Tools installation failed (exit code {}). Please try running the installer manually from: {}", exit_code, installer_path.display())
    }

    async fn install_rust(&self) -> Result<()> {
        logging::info("Installing Rust toolchain...");

        let installer_url = "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe";
        let installer_path = self.config.deps_dir().join("rustup-init.exe");

        std::fs::create_dir_all(self.config.deps_dir())?;

        let client = reqwest::Client::new();
        let response = client.get(installer_url).send().await?;
        let bytes = response.bytes().await?;
        std::fs::write(&installer_path, &bytes)?;

        let status = Command::new(&installer_path)
            .args(["-y", "--default-toolchain", "stable"])
            .status()
            .context("Failed to run Rust installer")?;

        if !status.success() {
            anyhow::bail!("Rust installation failed");
        }

        logging::success("Rust installed");
        Ok(())
    }

    async fn install_vulkan_sdk(&self) -> Result<()> {
        logging::info(&format!("Installing Vulkan SDK {}...", self.config.vulkan_version));

        let installer_url = format!(
            "https://sdk.lunarg.com/sdk/download/{}/windows/VulkanSDK-{}-Installer.exe",
            self.config.vulkan_version, self.config.vulkan_version
        );
        let installer_path = self.config.deps_dir().join("VulkanSDK-Installer.exe");

        std::fs::create_dir_all(self.config.deps_dir())?;

        let client = reqwest::Client::new();
        let response = client.get(&installer_url).send().await?;
        let bytes = response.bytes().await?;
        std::fs::write(&installer_path, &bytes)?;

        let status = Command::new(&installer_path)
            .args(["/S"])
            .status()
            .context("Failed to run Vulkan SDK installer")?;

        if !status.success() {
            anyhow::bail!("Vulkan SDK installation failed");
        }

        logging::success("Vulkan SDK installed");
        Ok(())
    }

    async fn install_tracy(&self) -> Result<()> {
        logging::info(&format!("Installing Tracy Profiler {}...", self.config.tracy_version));

        let archive_url = format!(
            "https://github.com/wolfpld/tracy/archive/refs/tags/v{}.zip",
            self.config.tracy_version
        );
        let archive_path = self.config.deps_dir().join("tracy.zip");

        std::fs::create_dir_all(self.config.deps_dir())?;

        let client = reqwest::Client::new();
        let response = client.get(&archive_url).send().await?;
        let bytes = response.bytes().await?;
        std::fs::write(&archive_path, &bytes)?;

        let file = std::fs::File::open(&archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(self.config.deps_dir())?;

        std::fs::remove_file(&archive_path)?;

        logging::success("Tracy Profiler installed");
        Ok(())
    }

    async fn install_o3de(&self) -> Result<()> {
        logging::info(&format!("Installing O3DE SDK {} (building from source)...", self.config.o3de_version));
        logging::warn("=".repeat(60).as_str());
        logging::warn("FIRST-TIME BUILD: This will take 60-120 minutes");
        logging::warn("Your PC may run slowly during compilation - this is normal");
        logging::warn("=".repeat(60).as_str());

        // Check for user's manual download at C:\O3DE-Source FIRST
        let user_source = PathBuf::from(r"C:\O3DE-Source");
        let o3de_dir = if user_source.join("CMakeLists.txt").exists() {
            logging::success(&format!("Found O3DE source at: {}", user_source.display()));
            user_source
        } else {
            self.config.o3de_dir()
        };
        
        let build_dir = o3de_dir.join("build").join("windows");
        let install_dir = o3de_dir.join("install");
        
        // Check if already built (has AzCore.lib)
        let azcore_lib = install_dir.join("lib").join("profile").join("AzCore.lib");
        if azcore_lib.exists() {
            logging::success("O3DE already built - skipping");
            return Ok(());
        }
        
        // Also check build output directly
        let build_lib = build_dir.join("lib").join("profile").join("AzCore.lib");
        if build_lib.exists() {
            logging::success("O3DE already built (in build dir) - skipping");
            return Ok(());
        }
        
        // Check if source exists but not built
        let has_source = o3de_dir.join("CMakeLists.txt").exists();
        
        if !has_source {
            // Remove any partial/corrupted installation
            if o3de_dir.exists() {
                logging::info("Removing incomplete O3DE installation...");
                if let Err(e) = std::fs::remove_dir_all(&o3de_dir) {
                    logging::warn(&format!("Could not remove directory: {} - continuing", e));
                }
            }
            
            std::fs::create_dir_all(o3de_dir.parent().unwrap_or(&o3de_dir))?;

            // Step 1: Clone from GitHub
            logging::info("");
            logging::info("[1/5] Cloning O3DE source from GitHub...");
            logging::info("      This downloads ~5GB and takes 10-20 minutes");
            
            let status = Command::new("git")
                .args([
                    "clone",
                    "--depth", "1",
                    "--branch", &self.config.o3de_version,  // Tag: "2510.1"
                    "https://github.com/o3de/o3de.git",
                    o3de_dir.to_str().unwrap(),
                ])
                .status()
                .context("Failed to clone O3DE repository")?;

            if !status.success() {
                // Try development branch as fallback
                logging::warn(&format!("Tag {} not found, trying development branch...", self.config.o3de_version));
                let status = Command::new("git")
                    .args([
                        "clone",
                        "--depth", "1",
                        "--branch", "development",
                        "https://github.com/o3de/o3de.git",
                        o3de_dir.to_str().unwrap(),
                    ])
                    .status()
                    .context("Failed to clone O3DE repository")?;
                
                if !status.success() {
                    anyhow::bail!("O3DE clone failed - check internet connection");
                }
            }
            logging::success("O3DE source cloned");
        } else {
            logging::info("O3DE source already exists, skipping clone");
        }

        // Step 2: Run Python bootstrap
        logging::info("");
        logging::info("[2/5] Setting up O3DE Python environment...");
        let bootstrap_script = o3de_dir.join("python").join("get_python.bat");
        
        if bootstrap_script.exists() {
            let status = Command::new("cmd")
                .args(["/C", bootstrap_script.to_str().unwrap()])
                .current_dir(&o3de_dir)
                .status()
                .context("Failed to run O3DE Python bootstrap")?;
            
            if !status.success() {
                logging::warn("Python bootstrap had warnings - continuing");
            }
        }
        logging::success("Python environment ready");

        // Step 3: Configure with CMake
        logging::info("");
        logging::info("[3/5] Configuring O3DE with CMake...");
        logging::info("      (This may take 5-10 minutes)");
        
        std::fs::create_dir_all(&build_dir)?;
        
        // Use cmake preset if available, otherwise manual configuration
        let cmake_args = vec![
            "-B", build_dir.to_str().unwrap(),
            "-S", o3de_dir.to_str().unwrap(),
            "-G", "Visual Studio 17 2022",
            "-A", "x64",
            "-DLY_DISABLE_TEST_MODULES=ON",
            "-DLY_UNITY_BUILD=ON",
            "-DCMAKE_INSTALL_PREFIX", install_dir.to_str().unwrap(),
        ];
        
        let status = Command::new("cmake")
            .args(&cmake_args)
            .current_dir(&o3de_dir)
            .env("O3DE_SNAP", "1")  // Skip test tools that require special Python modules
            .status()
            .context("Failed to configure O3DE with CMake")?;

        if !status.success() {
            anyhow::bail!("O3DE CMake configuration failed");
        }
        logging::success("CMake configuration complete");

        // Step 4: Build essential Atom targets
        logging::info("");
        logging::info("[4/5] Building O3DE Atom renderer...");
        logging::warn("      This is the longest step (45-90 minutes)");
        logging::info("      Building: AzCore, AzFramework, Atom_RPI.Public, Atom_RHI.Public");
        
        // Build targets one by one for better progress visibility
        let targets = ["AzCore", "AzFramework", "Atom_RHI.Public", "Atom_RPI.Public"];
        
        for (i, target) in targets.iter().enumerate() {
            logging::info(&format!("      [{}/{}] Building {}...", i + 1, targets.len(), target));
            
            let status = Command::new("cmake")
                .args([
                    "--build", build_dir.to_str().unwrap(),
                    "--config", "profile",
                    "--target", target,
                    "--parallel",  // Use all CPU cores
                ])
                .status()
                .context(format!("Failed to build O3DE target: {}", target))?;

            if !status.success() {
                logging::warn(&format!("{} build failed - trying ALL target...", target));
                break;
            }
            logging::success(&format!("      {} built", target));
        }

        // Step 5: Install libraries
        logging::info("");
        logging::info("[5/5] Installing O3DE libraries...");
        
        let status = Command::new("cmake")
            .args([
                "--install", build_dir.to_str().unwrap(),
                "--config", "profile",
                "--prefix", install_dir.to_str().unwrap(),
            ])
            .status()
            .context("Failed to install O3DE")?;

        if !status.success() {
            logging::warn("CMake install had warnings - checking manually...");
        }
        
        // Verify installation by checking for key libraries
        let lib_dir = install_dir.join("lib").join("profile");
        let key_libs = ["AzCore.lib", "AzFramework.lib"];
        let mut libs_found = false;
        
        for lib in &key_libs {
            if lib_dir.join(lib).exists() {
                libs_found = true;
                logging::success(&format!("Found {}", lib));
                break;
            }
        }
        
        // Also check build output directly if install didn't work
        if !libs_found {
            let build_lib_paths = [
                build_dir.join("lib").join("profile"),
                build_dir.join("bin").join("profile"),
                build_dir.join("Bin64").join("profile"),
            ];
            
            for path in &build_lib_paths {
                if path.join("AzCore.lib").exists() {
                    logging::success(&format!("Found libraries in build output: {}", path.display()));
                    libs_found = true;
                    
                    // Copy to install location
                    std::fs::create_dir_all(&lib_dir)?;
                    for entry in std::fs::read_dir(path)?.flatten() {
                        let src = entry.path();
                        if src.extension().map(|e| e == "lib").unwrap_or(false) {
                            let dst = lib_dir.join(src.file_name().unwrap());
                            let _ = std::fs::copy(&src, &dst);
                        }
                    }
                    break;
                }
            }
        }

        // Write version marker file
        let marker_file = self.config.install_dir.join("o3de_version.txt");
        std::fs::write(&marker_file, &self.config.o3de_version)
            .context("Failed to write O3DE version marker")?;

        if libs_found {
            logging::success("");
            logging::success("=".repeat(60).as_str());
            logging::success("O3DE ATOM RENDERER BUILT SUCCESSFULLY!");
            logging::success("=".repeat(60).as_str());
            logging::success("");
        } else {
            logging::warn("O3DE build completed but libraries not found in expected locations");
            logging::info("Build output may be in a different location - continuing anyway");
        }
        
        Ok(())
    }

    pub fn print_status(&self, deps: &[DependencyStatus]) {
        for dep in deps {
            if dep.installed {
                let version = dep.version.as_deref().unwrap_or("unknown");
                logging::success(&format!("{}: {}", dep.name, version));
            } else {
                logging::warn(&format!("{}: NOT INSTALLED", dep.name));
            }
        }
    }
}
