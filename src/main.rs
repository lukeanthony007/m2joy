mod config;
mod mouse;
mod virtual_pad;

use clap::Parser;
use config::Config;
use mouse::{find_mouse_device, MouseReader, MouseState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use virtual_pad::VirtualPad;

static QUIT: AtomicBool = AtomicBool::new(false);
pub(crate) static TOGGLE: AtomicBool = AtomicBool::new(false);

/// Base scale factor for mapping mouse velocity to stick deflection.
/// Tuned so sensitivity=1.0 gives reasonable response at 800-1600 DPI.
const BASE_SCALE: f32 = 5000.0;

fn main() {
    // Handle "m2joy toggle" / "m2joy quit" before clap parsing.
    // These send a signal to the running instance and exit immediately.
    if let Some(cmd) = std::env::args().nth(1) {
        match cmd.as_str() {
            "toggle" => {
                send_to_running(libc::SIGUSR1, "Toggle");
                return;
            }
            "quit" => {
                send_to_running(libc::SIGTERM, "Quit");
                return;
            }
            _ => {}
        }
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let config = Config::parse();

    println!("m2joy - Mouse-to-Joystick for RetroArch");
    println!("  Sensitivity: {:.2}", config.sensitivity);
    println!("  Invert Y:    {}", config.invert_y);
    println!("  Output:      {} stick", if config.left_stick { "left" } else { "right" });
    println!("  Decay:       {:.3}", config.decay);
    println!();

    signal_setup();

    // Find mouse device
    let device_path = match &config.device {
        Some(path) => path.clone(),
        None => match find_mouse_device() {
            Some(p) => {
                let s = p.to_string_lossy().to_string();
                log::info!("Auto-detected mouse: {}", s);
                s
            }
            None => {
                log::error!("No mouse device found. Are you in the 'input' group?");
                log::error!("Try: sudo usermod -aG input $USER (then re-login)");
                std::process::exit(1);
            }
        },
    };

    // Create virtual gamepad
    let mut pad = match VirtualPad::new(config.left_stick) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to create virtual gamepad: {}", e);
            log::error!("Do you have /dev/uinput access? Try: sudo modprobe uinput");
            std::process::exit(1);
        }
    };

    // Spawn mouse reader thread
    let mouse_state = Arc::new(MouseState::new());
    let mouse_state_clone = Arc::clone(&mouse_state);
    let device_path_clone = device_path.clone();

    let mouse_thread = std::thread::Builder::new()
        .name("mouse-reader".into())
        .spawn(move || {
            match MouseReader::new(&device_path_clone, mouse_state_clone) {
                Ok(mut reader) => reader.run(),
                Err(e) => {
                    log::error!("Failed to open mouse device: {}", e);
                    log::error!("Check permissions on {}", device_path_clone);
                }
            }
        })
        .expect("Failed to spawn mouse thread");

    println!("Toggle: m2joy toggle");
    println!("Quit:   m2joy quit");
    println!("Configure RetroArch to use 'm2joy Stick' as a controller.");
    println!();

    // Main 1kHz loop with leaky accumulator
    let tick = Duration::from_micros(1000);
    let decay = config.decay.clamp(0.5, 0.999);
    let scale = BASE_SCALE * config.sensitivity;
    let y_sign = if config.invert_y { -1.0f32 } else { 1.0 };

    let mut accum_x: f32 = 0.0;
    let mut accum_y: f32 = 0.0;

    loop {
        let tick_start = std::time::Instant::now();

        if QUIT.load(Ordering::Relaxed) || mouse_state.quit.load(Ordering::Relaxed) {
            break;
        }

        if mouse_state.active.load(Ordering::Relaxed) {
            let (dx, dy) = mouse_state.drain();

            // Leaky accumulator: smooths mouse velocity into stick position
            accum_x = accum_x * decay + dx as f32;
            accum_y = accum_y * decay + dy as f32 * y_sign;

            // Snap tiny residuals to zero
            if accum_x.abs() < 0.01 {
                accum_x = 0.0;
            }
            if accum_y.abs() < 0.01 {
                accum_y = 0.0;
            }

            let sx = (accum_x * scale) as i32;
            let sy = (accum_y * scale) as i32;

            if let Err(e) = pad.emit_stick(sx, sy) {
                log::warn!("Failed to emit stick: {}", e);
            }
        } else {
            // Not active — reset accumulator and center stick
            if accum_x != 0.0 || accum_y != 0.0 {
                accum_x = 0.0;
                accum_y = 0.0;
                let _ = pad.emit_stick(0, 0);
            }
        }

        let elapsed = tick_start.elapsed();
        if elapsed < tick {
            spin_sleep::sleep(tick - elapsed);
        }
    }

    // Center stick before exit
    let _ = pad.emit_stick(0, 0);

    log::info!("Shutting down...");
    mouse_state.quit.store(true, Ordering::Relaxed);
    let _ = mouse_thread.join();
    log::info!("Done");
}

fn signal_setup() {
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGUSR1, signal_handler as libc::sighandler_t);
    }
}

extern "C" fn signal_handler(sig: libc::c_int) {
    match sig {
        libc::SIGUSR1 => TOGGLE.store(true, Ordering::Relaxed),
        _ => QUIT.store(true, Ordering::Relaxed),
    }
}

/// Find PID of a running m2joy instance by scanning /proc.
fn find_running_instance() -> Option<i32> {
    let my_pid = std::process::id() as i32;
    for entry in std::fs::read_dir("/proc").ok()? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let pid: i32 = match entry.file_name().to_str().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        if pid == my_pid {
            continue;
        }
        if let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) {
            if comm.trim() == "m2joy" {
                return Some(pid);
            }
        }
    }
    None
}

/// Send a signal to the running m2joy instance, or exit with an error.
fn send_to_running(sig: libc::c_int, action: &str) {
    match find_running_instance() {
        Some(pid) => {
            let ret = unsafe { libc::kill(pid, sig) };
            if ret == 0 {
                eprintln!("{} sent to m2joy (pid {})", action, pid);
            } else {
                eprintln!("Failed to send signal to m2joy (pid {})", pid);
                std::process::exit(1);
            }
        }
        None => {
            eprintln!("No running m2joy instance found");
            std::process::exit(1);
        }
    }
}
