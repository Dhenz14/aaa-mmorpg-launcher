mod config;
mod dependencies;
mod logging;
mod orchestrator;
mod state_machine;
mod sync;
mod updater;

use anyhow::Result;
use state_machine::{LauncherState, StateMachine};
use std::io::Write;

use crate::config::Config;
use crate::dependencies::DependencyManager;
use crate::orchestrator::BuildOrchestrator;
use crate::sync::SyncManager;
use crate::updater::Updater;

struct Args {
    help: bool,
    version: bool,
    dry_run: bool,
    verbose: bool,
    skip_elevation: bool,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    Args {
        help: args.iter().any(|a| a == "--help" || a == "-h"),
        version: args.iter().any(|a| a == "--version" || a == "-V"),
        dry_run: args.iter().any(|a| a == "--dry-run" || a == "--test"),
        verbose: args.iter().any(|a| a == "--verbose" || a == "-v"),
        skip_elevation: args.iter().any(|a| a == "--skip-elevation"),
    }
}

fn print_help() {
    println!("AAA MMORPG Engine Launcher v{}", config::LAUNCHER_VERSION);
    println!();
    println!("USAGE:");
    println!("    aaa-launcher.exe [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help           Show this help message");
    println!("    -V, --version        Show version");
    println!("    -v, --verbose        Enable verbose logging");
    println!("    --dry-run            Test mode (check deps, don't build)");
    println!("    --skip-elevation     Don't request admin rights");
    println!();
}

fn print_version() {
    println!("aaa-launcher {}", config::LAUNCHER_VERSION);
}

#[cfg(windows)]
fn is_elevated() -> bool {
    use std::mem;
    use std::ptr;
    use winapi::ctypes::c_void;
    
    unsafe {
        let mut token: *mut c_void = ptr::null_mut();
        let process = winapi::um::processthreadsapi::GetCurrentProcess();
        
        if winapi::um::processthreadsapi::OpenProcessToken(
            process,
            winapi::um::winnt::TOKEN_QUERY,
            &mut token,
        ) == 0 {
            return false;
        }
        
        let mut elevation: winapi::um::winnt::TOKEN_ELEVATION = mem::zeroed();
        let mut size: u32 = 0;
        
        let result = winapi::um::securitybaseapi::GetTokenInformation(
            token,
            winapi::um::winnt::TokenElevation,
            &mut elevation as *mut _ as *mut c_void,
            mem::size_of::<winapi::um::winnt::TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        
        winapi::um::handleapi::CloseHandle(token);
        
        result != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(windows)]
fn request_elevation() -> bool {
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;
    use std::iter::once;
    
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    
    let exe_wide: Vec<u16> = OsStr::new(exe_path.to_str().unwrap_or(""))
        .encode_wide()
        .chain(once(0))
        .collect();
    
    let verb: Vec<u16> = OsStr::new("runas")
        .encode_wide()
        .chain(once(0))
        .collect();
    
    let args: Vec<u16> = OsStr::new("--skip-elevation")
        .encode_wide()
        .chain(once(0))
        .collect();
    
    unsafe {
        let result = winapi::um::shellapi::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            exe_wide.as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            winapi::um::winuser::SW_SHOWNORMAL,
        );
        
        (result as isize) > 32
    }
}

#[cfg(not(windows))]
fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(windows))]
fn request_elevation() -> bool {
    false
}

fn wait_for_enter() {
    println!();
    println!("Press Enter to exit...");
    let _ = std::io::stdout().flush();
    let _ = std::io::stdin().read_line(&mut String::new());
}

#[tokio::main]
async fn main() {
    let args = parse_args();
    
    if args.help {
        print_help();
        return;
    }
    
    if args.version {
        print_version();
        return;
    }
    
    // Early logging to console before config is loaded
    println!();
    println!("AAA MMORPG Engine Launcher v{}", config::LAUNCHER_VERSION);
    println!("=====================================");
    println!();
    
    // Check elevation on Windows
    #[cfg(windows)]
    if !args.skip_elevation && !is_elevated() {
        println!("Requesting administrator privileges...");
        println!("(Required for installing Vulkan SDK and VS Build Tools)");
        println!();
        
        if request_elevation() {
            println!("Elevated process started. This window will close.");
            std::thread::sleep(std::time::Duration::from_secs(2));
            return;
        } else {
            println!("WARNING: Could not elevate. Some features may not work.");
            println!("Continuing without admin rights...");
            println!();
        }
    }
    
    match run(args).await {
        Ok(()) => {
            println!();
            println!("Launcher completed successfully.");
            wait_for_enter();
        }
        Err(e) => {
            eprintln!();
            eprintln!("=====================================");
            eprintln!("ERROR: {:#}", e);
            eprintln!("=====================================");
            eprintln!();
            wait_for_enter();
            std::process::exit(1);
        }
    }
}

async fn run(args: Args) -> Result<()> {
    let mut config = Config::load()?;
    config.verbose = args.verbose;
    
    // Create directories first so logging can work
    std::fs::create_dir_all(&config.install_dir)?;
    std::fs::create_dir_all(&config.logs_dir())?;
    
    logging::init(&config.logs_dir(), config.verbose)?;
    logging::header();
    
    println!("Install directory: {}", config.install_dir.display());
    println!("Server: {}", config.server_url);
    println!("Log directory: {}", config.logs_dir().display());
    println!();

    let mut state_machine = StateMachine::new(&config.install_dir)?;

    if state_machine.current() == LauncherState::Complete {
        state_machine.reset()?;
    }

    loop {
        let current_state = state_machine.current();
        let step = current_state.step_number();
        let total = LauncherState::total_steps();

        logging::step(step, total, &current_state.to_string());

        let result = match current_state {
            LauncherState::Init => run_init(&config).await,
            LauncherState::SelfUpdate => run_self_update(&config).await,
            LauncherState::DependencyAudit => run_dependency_audit(&config, args.dry_run).await,
            LauncherState::Sync => {
                if args.dry_run {
                    logging::info("Dry-run mode: skipping sync");
                    Ok(())
                } else {
                    run_sync(&config).await
                }
            }
            LauncherState::Build => {
                if args.dry_run {
                    logging::info("Dry-run mode: skipping build");
                    Ok(())
                } else {
                    run_build(&config).await
                }
            }
            LauncherState::Launch => {
                if args.dry_run {
                    logging::info("Dry-run mode: skipping launch");
                    Ok(())
                } else {
                    run_launch(&config).await
                }
            }
            LauncherState::Complete => break,
            LauncherState::Failed => {
                logging::error("Previous run failed - resetting state");
                state_machine.reset()?;
                continue;
            }
        };

        match result {
            Ok(()) => {
                if state_machine.transition()?.is_none() {
                    break;
                }
            }
            Err(e) => {
                logging::error(&format!("{:#}", e));
                state_machine.fail()?;
                return Err(e);
            }
        }
    }

    state_machine.clear_saved_state()?;
    
    if args.dry_run {
        logging::success("Dry-run completed successfully!");
        logging::info("All checks passed. Run without --dry-run to perform full installation.");
    } else {
        logging::complete();
    }

    Ok(())
}

async fn run_init(config: &Config) -> Result<()> {
    logging::info(&format!("Install directory: {}", config.install_dir.display()));
    logging::info(&format!("Server: {}", config.server_url));
    
    std::fs::create_dir_all(&config.install_dir)?;
    std::fs::create_dir_all(&config.deps_dir())?;
    std::fs::create_dir_all(&config.logs_dir())?;
    
    logging::success("Directories initialized");
    Ok(())
}

async fn run_self_update(config: &Config) -> Result<()> {
    if config.skip_update {
        logging::info("Update check skipped");
        return Ok(());
    }

    let updater = Updater::new(config.clone())?;
    
    match updater.check_for_update().await? {
        Some(update_info) => {
            let temp_path = config.install_dir.join("launcher_update.exe");
            
            updater.download_and_verify(&temp_path, &update_info.checksum).await?;
            
            let current_exe = std::env::current_exe()?;
            Updater::apply_update(&temp_path, &current_exe)?;
            
            Updater::request_restart();
        }
        None => {
            logging::success("Launcher is up to date");
        }
    }

    Ok(())
}

async fn run_dependency_audit(config: &Config, dry_run: bool) -> Result<()> {
    let dep_manager = DependencyManager::new(config.clone());
    let deps = dep_manager.check_all();

    dep_manager.print_status(&deps);

    let missing: Vec<_> = deps.iter().filter(|d| !d.installed).collect();
    
    if missing.is_empty() {
        logging::success("All dependencies satisfied");
    } else {
        logging::warn(&format!("{} dependencies need installation", missing.len()));
        
        if dry_run {
            logging::info("Dry-run mode: would install:");
            for dep in &missing {
                logging::info(&format!("  - {}", dep.name));
            }
        } else {
            for dep in &missing {
                logging::info(&format!("Installing: {}", dep.name));
            }
            
            dep_manager.install_missing(&deps).await?;
            
            let recheck = dep_manager.check_all();
            let still_missing: Vec<_> = recheck.iter().filter(|d| !d.installed).collect();
            
            if !still_missing.is_empty() {
                anyhow::bail!(
                    "Failed to install dependencies: {:?}",
                    still_missing.iter().map(|d| &d.name).collect::<Vec<_>>()
                );
            }
            
            logging::success("All dependencies installed");
        }
    }

    Ok(())
}

async fn run_sync(config: &Config) -> Result<()> {
    let sync_manager = SyncManager::new(config.clone())?;
    
    let _server_version = sync_manager.check_server().await?;
    
    let engine_dir = config.engine_dir();
    if !engine_dir.exists() || std::fs::read_dir(&engine_dir)?.count() == 0 {
        logging::info("No local files - downloading full archive");
        sync_manager.download_full_archive().await?;
    } else {
        match sync_manager.get_manifest().await {
            Ok(manifest) => {
                sync_manager.sync_files(&manifest).await?;
            }
            Err(e) => {
                logging::warn(&format!("Could not get manifest: {} - using full sync", e));
                sync_manager.download_full_archive().await?;
            }
        }
    }

    Ok(())
}

async fn run_build(config: &Config) -> Result<()> {
    let orchestrator = BuildOrchestrator::new(config.clone());
    
    if orchestrator.needs_rebuild()? {
        orchestrator.run_build()?;
        orchestrator.save_build_version()?;
    } else {
        logging::success("Build cache valid - skipping rebuild");
    }

    // Build Render Fabric and run validation tests
    orchestrator.build_render_fabric()?;
    orchestrator.run_validation_tests()?;

    Ok(())
}

async fn run_launch(config: &Config) -> Result<()> {
    let orchestrator = BuildOrchestrator::new(config.clone());
    orchestrator.launch_game()?;
    
    logging::success("Game launched");
    Ok(())
}
