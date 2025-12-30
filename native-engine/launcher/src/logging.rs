use anyhow::Result;
use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "");
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static CROSS: Emoji<'_, '_> = Emoji("❌ ", "[ERR] ");
static WARN: Emoji<'_, '_> = Emoji("⚠️  ", "[WARN] ");
static GEAR: Emoji<'_, '_> = Emoji("⚙️  ", "[...] ");
static DOWNLOAD: Emoji<'_, '_> = Emoji("📥 ", "[DL] ");

pub fn init(logs_dir: &Path, verbose: bool) -> Result<()> {
    std::fs::create_dir_all(logs_dir)?;
    
    let log_file = logs_dir.join(format!(
        "launcher_{}.log",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    
    let file_appender = std::fs::File::create(&log_file)?;
    
    let filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_target(false)
                .without_time()
        )
        .with(
            fmt::layer()
                .with_writer(std::sync::Mutex::new(file_appender))
                .with_ansi(false)
        )
        .init();

    Ok(())
}

pub fn header() {
    println!();
    println!("{}", style("═══════════════════════════════════════════════════════════════").cyan());
    println!("{}", style("     AAA MMORPG ENGINE - Professional Launcher").cyan().bold());
    println!("{}", style(format!("     Version {} | O3DE Atom | No Fallbacks", crate::config::LAUNCHER_VERSION)).cyan());
    println!("{}", style("═══════════════════════════════════════════════════════════════").cyan());
    println!();
}

pub fn step(number: u8, total: u8, message: &str) {
    println!(
        "{} {} {}",
        style(format!("[{}/{}]", number, total)).bold().cyan(),
        GEAR,
        style(message).bold()
    );
}

pub fn success(message: &str) {
    println!("       {}{}", CHECK, style(message).green());
}

pub fn error(message: &str) {
    println!("       {}{}", CROSS, style(message).red());
}

pub fn warn(message: &str) {
    println!("       {}{}", WARN, style(message).yellow());
}

pub fn info(message: &str) {
    println!("       {}", style(message).dim());
}

pub fn download(message: &str) {
    println!("       {}{}", DOWNLOAD, message);
}

pub fn progress_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("       [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb
}

#[allow(dead_code)]
pub fn spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("       {spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

pub fn fatal(message: &str) -> ! {
    println!();
    println!("{}", style("═══════════════════════════════════════════════════════════════").red());
    println!("{} {}", CROSS, style("FATAL ERROR").red().bold());
    println!("{}", style("═══════════════════════════════════════════════════════════════").red());
    println!();
    println!("  {}", message);
    println!();
    println!("  Press Enter to exit...");
    let _ = std::io::stdin().read_line(&mut String::new());
    std::process::exit(1);
}

pub fn complete() {
    println!();
    println!("{}", style("═══════════════════════════════════════════════════════════════").green());
    println!("{} {}", ROCKET, style("ENGINE LAUNCHED SUCCESSFULLY").green().bold());
    println!("{}", style("═══════════════════════════════════════════════════════════════").green());
    println!();
}
