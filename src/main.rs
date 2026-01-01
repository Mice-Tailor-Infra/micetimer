use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, error, info};
use nix::sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags};
use nix::sys::time::TimeSpec;
use nix::sys::timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags};
use serde::Deserialize;
use std::fs;
use std::os::unix::io::{AsFd, AsRawFd};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

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
#[derive(Debug, Deserialize, Clone)]
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

/// Active timer runtime state
struct RuntimeTimer {
    name: String,
    unit: TimerUnit,
    tfd: TimerFd,
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

fn execute_timer(timer: &RuntimeTimer) {
    info!("Executing [{}]: {}", timer.name, timer.unit.exec);

    let lock_name = format!("micetimer:{}", timer.name);

    let mut use_wakelock = false;

    // Acquire Android WakeLock

    if timer.unit.wake_lock {
        match fs::write("/sys/power/wake_lock", &lock_name) {
            Ok(_) => {
                debug!("Acquired WakeLock: {}", lock_name);

                use_wakelock = true;
            }

            Err(e) => error!("Failed to acquire WakeLock {}: {}", lock_name, e),
        }
    }

    let status = Command::new("sh").arg("-c").arg(&timer.unit.exec).status();

    match status {
        Ok(s) => {
            if s.success() {
                info!("Finished [{}]: Success", timer.name);
            } else {
                error!(
                    "Finished [{}]: Failed with exit code {:?}",
                    timer.name,
                    s.code()
                );
            }
        }

        Err(e) => {
            error!("Finished [{}]: Error executing command: {}", timer.name, e);
        }
    }

    // Release Android WakeLock

    if use_wakelock {
        if let Err(e) = fs::write("/sys/power/wake_unlock", &lock_name) {
            error!("Failed to release WakeLock {}: {}", lock_name, e);
        } else {
            debug!("Released WakeLock: {}", lock_name);
        }
    }
}

fn main() -> Result<()> {
    // Initialize logger
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Debug,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )
    .unwrap();

    let args = Args::parse();

    info!("MiceTimer Daemon starting...");
    info!("Configuration directory: {}", args.config_dir);

    // Load timer definitions
    let timer_units = load_timers(&args.config_dir)?;

    if timer_units.is_empty() {
        info!("No timer configurations found in {}", args.config_dir);
        return Ok(());
    }

    info!(
        "Loaded {} timer(s), initializing event loop...",
        timer_units.len()
    );

    let epoll = Epoll::new(EpollCreateFlags::empty())?;
    let mut active_timers = std::collections::HashMap::new();

    for (name, unit) in timer_units {
        // Create TimerFd with CLOCK_BOOTTIME (crucial for Android/deep sleep)
        let tfd = TimerFd::new(
            ClockId::CLOCK_BOOTTIME,
            TimerFlags::TFD_NONBLOCK | TimerFlags::TFD_CLOEXEC,
        )?;

        let initial_delay = unit.on_boot_sec.unwrap_or(Duration::from_secs(1));

        // We use OneShot and re-arm manually to have full control and potentially different interval logic
        tfd.set(
            Expiration::OneShot(TimeSpec::from(initial_delay)),
            TimerSetTimeFlags::empty(),
        )?;

        let fd = tfd.as_fd().as_raw_fd();
        let event = EpollEvent::new(EpollFlags::EPOLLIN, fd as u64);
        epoll.add(&tfd, event)?;

        active_timers.insert(fd, RuntimeTimer { name, unit, tfd });
    }

    info!("Event loop started. Waiting for triggers...");

    let mut events = [EpollEvent::empty(); 16];
    loop {
        match epoll.wait(&mut events, -1) {
            Ok(num_events) => {
                for i in 0..num_events {
                    let fd = events[i].data() as i32;
                    if let Some(timer) = active_timers.get_mut(&fd) {
                        // Read from timerfd to clear the trigger
                        let _ = timer.tfd.wait();

                        // Execute the command
                        execute_timer(timer);

                        // Re-arm if it's a repeating timer
                        if let Some(interval) = timer.unit.on_unit_active_sec {
                            if interval > Duration::ZERO {
                                debug!("Re-arming [{}] for {:?}", timer.name, interval);
                                if let Err(e) = timer.tfd.set(
                                    Expiration::OneShot(TimeSpec::from(interval)),
                                    TimerSetTimeFlags::empty(),
                                ) {
                                    error!("Failed to re-arm [{}]: {}", timer.name, e);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) if e == nix::Error::EINTR => continue,
            Err(e) => return Err(e.into()),
        }
    }
}
