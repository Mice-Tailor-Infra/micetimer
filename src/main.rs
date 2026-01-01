use anyhow::{Context, Result};
use clap::Parser;
use log::{info, debug, error};
use serde::Deserialize;
use std::time::Duration;
use std::path::Path;
use std::fs;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory containing timer configurations
    #[arg(short, long, default_value = "/data/adb/micetimer/timers.d")]
    config_dir: String,

    /// Run in foreground (don't daemonize) - useful for debugging
    #[arg(short, long)]
    foreground: bool,
}

/// Represents a single timer unit configuration (one file = one unit)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")] // Match Systemd naming convention (e.g., Exec, OnBootSec)
struct TimerUnit {
    description: Option<String>,
    
    /// Command to execute
    exec: String,
    
    /// Active wait after boot
    #[serde(default, with = "humantime_serde")]
    on_boot_sec: Option<Duration>,
    
    /// Repeat interval relative to the last activation
    #[serde(default, with = "humantime_serde")]
    on_unit_active_sec: Option<Duration>,
    
    /// Whether to hold a partial wakelock during execution
    #[serde(default = "default_wakelock")]
    wake_lock: bool,
}

fn default_wakelock() -> bool {
    true
}

/// Scans the configuration directory for .toml files
fn load_timers<P: AsRef<Path>>(dir: P) -> Result<Vec<(String, TimerUnit)>> {
    let mut timers = Vec::new();
    let path_ref = dir.as_ref();
    
    if !path_ref.exists() {
        // Just return empty if dir doesn't exist yet
        return Ok(timers); 
    }

    for entry in fs::read_dir(path_ref)? {
        let entry = entry?;
        let path = entry.path();
        
        // Only process .toml files
        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let content = fs::read_to_string(&path)?;
            
            // Parse with context for better error messages
            let unit: TimerUnit = toml::from_str(&content)
                .with_context(|| format!("Failed to parse configuration: {:?}", path))?;
                
            timers.push((name, unit));
        }
    }
    Ok(timers)
}

fn main() -> Result<()> {
    // Initialize logger
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Debug,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    ).unwrap();

    let args = Args::parse();

    info!("MiceTimer Daemon starting...");
    info!("Configuration directory: {}", args.config_dir);

    // Load timer definitions
    let timers = load_timers(&args.config_dir)?;
    
    if timers.is_empty() {
        info!("No timer configurations found in {}", args.config_dir);
    } else {
        info!("Loaded {} timer(s):", timers.len());
        for (name, unit) in &timers {
            info!("  [+] {} - {}", name, unit.description.as_deref().unwrap_or("No description"));
            debug!("      Exec: {}, Interval: {:?}", unit.exec, unit.on_unit_active_sec);
        }
    }

    // TODO: Initialize timerfd and event loop here
    // The next step involves implementing the actual scheduling logic using `nix::sys::timerfd`.
    
    Ok(())
}
