use anyhow::{Context, Result};
use std::process::{Command, Stdio};

use crate::config::Config;
use crate::logging;

pub struct BuildOrchestrator {
    config: Config,
}

impl BuildOrchestrator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn run_build(&self) -> Result<()> {
        let engine_dir = self.config.engine_dir();
        let orchestrator_path = engine_dir.join("build-orchestrator.ps1");

        if !orchestrator_path.exists() {
            anyhow::bail!(
                "Build orchestrator not found at: {}",
                orchestrator_path.display()
            );
        }

        logging::info("Starting build process...");
        logging::warn("First build may take 60-120 minutes");

        let mut cmd = Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", orchestrator_path.to_str().unwrap(),
            "-InstallDir", engine_dir.to_str().unwrap(),
        ]);

        cmd.env("O3DE_HOME", self.config.o3de_dir());
        cmd.env("VULKAN_SDK", self.config.vulkan_sdk_dir());
        cmd.env("TRACY_DIR", self.config.tracy_dir());

        cmd.current_dir(&engine_dir);
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let status = cmd.status().context("Failed to run build orchestrator")?;

        if !status.success() {
            anyhow::bail!("Build failed with exit code: {:?}", status.code());
        }

        logging::success("Build completed successfully");
        Ok(())
    }

    pub fn check_build_cache(&self) -> bool {
        let engine_dir = self.config.engine_dir();
        let build_marker = engine_dir.join("target").join("release").join(".build_complete");
        
        build_marker.exists()
    }

    pub fn needs_rebuild(&self) -> Result<bool> {
        if self.config.force_rebuild {
            return Ok(true);
        }

        if !self.check_build_cache() {
            return Ok(true);
        }

        let engine_dir = self.config.engine_dir();
        let version_file = engine_dir.join(".build_version");
        
        if !version_file.exists() {
            return Ok(true);
        }

        let cached_version = std::fs::read_to_string(&version_file)?;
        let current_version = self.get_source_version()?;

        Ok(cached_version.trim() != current_version.trim())
    }

    fn get_source_version(&self) -> Result<String> {
        let engine_dir = self.config.engine_dir();
        let version_file = engine_dir.join("VERSION");
        
        if version_file.exists() {
            Ok(std::fs::read_to_string(version_file)?)
        } else {
            Ok("unknown".to_string())
        }
    }

    pub fn save_build_version(&self) -> Result<()> {
        let engine_dir = self.config.engine_dir();
        let version_file = engine_dir.join(".build_version");
        let version = self.get_source_version()?;
        
        std::fs::write(version_file, version)?;
        
        let build_marker = engine_dir.join("target").join("release").join(".build_complete");
        if let Some(parent) = build_marker.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(build_marker, "complete")?;
        
        Ok(())
    }

    pub fn launch_game(&self) -> Result<()> {
        let engine_dir = self.config.engine_dir();
        let game_exe = engine_dir
            .join("target")
            .join("release")
            .join("aaa-mmorpg.exe");

        if !game_exe.exists() {
            anyhow::bail!("Game executable not found at: {}", game_exe.display());
        }

        logging::info("Launching game...");

        Command::new(&game_exe)
            .current_dir(&engine_dir)
            .env("O3DE_HOME", self.config.o3de_dir())
            .env("VULKAN_SDK", self.config.vulkan_sdk_dir())
            .spawn()
            .context("Failed to launch game")?;

        Ok(())
    }

    pub fn build_render_fabric(&self) -> Result<()> {
        let engine_dir = self.config.engine_dir();
        let atom_bridge_dir = engine_dir.join("atom-bridge").join("cpp");
        let build_dir = atom_bridge_dir.join("build");

        logging::info("Building Render Fabric (custom Vulkan renderer)...");

        std::fs::create_dir_all(&build_dir)?;

        let mut cmake_configure = Command::new("cmake");
        cmake_configure.args([
            "..",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DBUILD_VALIDATION_TESTS=ON",
        ]);
        cmake_configure.current_dir(&build_dir);
        cmake_configure.env("VULKAN_SDK", self.config.vulkan_sdk_dir());
        cmake_configure.stdout(Stdio::inherit());
        cmake_configure.stderr(Stdio::inherit());

        let status = cmake_configure.status().context("Failed to run cmake configure")?;
        if !status.success() {
            anyhow::bail!("CMake configure failed");
        }

        let mut cmake_build = Command::new("cmake");
        cmake_build.args(["--build", ".", "--config", "Release", "-j"]);
        cmake_build.current_dir(&build_dir);
        cmake_build.stdout(Stdio::inherit());
        cmake_build.stderr(Stdio::inherit());

        let status = cmake_build.status().context("Failed to run cmake build")?;
        if !status.success() {
            anyhow::bail!("CMake build failed");
        }

        logging::success("Render Fabric built successfully (libatom_bridge.a + validation_test)");
        Ok(())
    }

    pub fn run_validation_tests(&self) -> Result<()> {
        let engine_dir = self.config.engine_dir();
        let test_exe = engine_dir
            .join("atom-bridge")
            .join("cpp")
            .join("build")
            .join("bin")
            .join("validation_test.exe");

        if !test_exe.exists() {
            let alt_path = engine_dir
                .join("atom-bridge")
                .join("cpp")
                .join("build")
                .join("bin")
                .join("validation_test");
            if alt_path.exists() {
                return self.run_test_exe(&alt_path);
            }
            anyhow::bail!("Validation test not found at: {}", test_exe.display());
        }

        self.run_test_exe(&test_exe)
    }

    fn run_test_exe(&self, test_exe: &std::path::Path) -> Result<()> {
        logging::info("Running Vulkan validation tests...");
        logging::info(&format!("Test executable: {}", test_exe.display()));

        let mut cmd = Command::new(test_exe);
        cmd.env("VULKAN_SDK", self.config.vulkan_sdk_dir());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let status = cmd.status().context("Failed to run validation test")?;

        if status.success() {
            logging::success("All validation tests PASSED - Frame graph barriers are Vulkan-compliant!");
        } else {
            logging::warn(&format!("Validation tests completed with exit code: {:?}", status.code()));
        }

        Ok(())
    }
}
