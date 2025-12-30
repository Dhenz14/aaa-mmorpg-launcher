use anyhow::{Context, Result};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherState {
    Init,
    SelfUpdate,
    DependencyAudit,
    Sync,
    Build,
    Launch,
    Complete,
    Failed,
}

impl fmt::Display for LauncherState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LauncherState::Init => write!(f, "Initializing"),
            LauncherState::SelfUpdate => write!(f, "Checking for Updates"),
            LauncherState::DependencyAudit => write!(f, "Verifying Dependencies"),
            LauncherState::Sync => write!(f, "Syncing Files"),
            LauncherState::Build => write!(f, "Building Engine"),
            LauncherState::Launch => write!(f, "Launching Game"),
            LauncherState::Complete => write!(f, "Complete"),
            LauncherState::Failed => write!(f, "Failed"),
        }
    }
}

impl LauncherState {
    pub fn next(self) -> Option<LauncherState> {
        match self {
            LauncherState::Init => Some(LauncherState::SelfUpdate),
            LauncherState::SelfUpdate => Some(LauncherState::DependencyAudit),
            LauncherState::DependencyAudit => Some(LauncherState::Sync),
            LauncherState::Sync => Some(LauncherState::Build),
            LauncherState::Build => Some(LauncherState::Launch),
            LauncherState::Launch => Some(LauncherState::Complete),
            LauncherState::Complete => None,
            LauncherState::Failed => None,
        }
    }

    pub fn step_number(self) -> u8 {
        match self {
            LauncherState::Init => 0,
            LauncherState::SelfUpdate => 1,
            LauncherState::DependencyAudit => 2,
            LauncherState::Sync => 3,
            LauncherState::Build => 4,
            LauncherState::Launch => 5,
            LauncherState::Complete => 6,
            LauncherState::Failed => 0,
        }
    }

    pub fn total_steps() -> u8 {
        6
    }
}

pub struct StateMachine {
    current_state: LauncherState,
    state_file: std::path::PathBuf,
}

impl StateMachine {
    pub fn new(install_dir: &std::path::Path) -> Result<Self> {
        let state_file = install_dir.join("launcher_state.json");
        let current_state = Self::load_state(&state_file).unwrap_or(LauncherState::Init);
        
        Ok(Self {
            current_state,
            state_file,
        })
    }

    fn load_state(path: &std::path::Path) -> Option<LauncherState> {
        let content = std::fs::read_to_string(path).ok()?;
        let data: serde_json::Value = serde_json::from_str(&content).ok()?;
        let state_str = data.get("state")?.as_str()?;
        
        match state_str {
            "Init" => Some(LauncherState::Init),
            "SelfUpdate" => Some(LauncherState::SelfUpdate),
            "DependencyAudit" => Some(LauncherState::DependencyAudit),
            "Sync" => Some(LauncherState::Sync),
            "Build" => Some(LauncherState::Build),
            "Launch" => Some(LauncherState::Launch),
            "Complete" => Some(LauncherState::Complete),
            "Failed" => Some(LauncherState::Failed),
            _ => None,
        }
    }

    fn save_state(&self) -> Result<()> {
        let data = serde_json::json!({
            "state": format!("{:?}", self.current_state),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        if let Some(parent) = self.state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        std::fs::write(&self.state_file, serde_json::to_string_pretty(&data)?)
            .context("Failed to save state")?;
        
        Ok(())
    }

    pub fn current(&self) -> LauncherState {
        self.current_state
    }

    pub fn transition(&mut self) -> Result<Option<LauncherState>> {
        if let Some(next) = self.current_state.next() {
            self.current_state = next;
            self.save_state()?;
            Ok(Some(next))
        } else {
            Ok(None)
        }
    }

    pub fn fail(&mut self) -> Result<()> {
        self.current_state = LauncherState::Failed;
        self.save_state()
    }

    pub fn reset(&mut self) -> Result<()> {
        self.current_state = LauncherState::Init;
        self.save_state()
    }

    #[allow(dead_code)]
    pub fn set_state(&mut self, state: LauncherState) -> Result<()> {
        self.current_state = state;
        self.save_state()
    }

    pub fn clear_saved_state(&self) -> Result<()> {
        if self.state_file.exists() {
            std::fs::remove_file(&self.state_file)?;
        }
        Ok(())
    }
}
