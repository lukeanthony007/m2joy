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

/// Scale factor mapping mouse velocity (sum of deltas over the window) to stick deflection.
const BASE_SCALE: f32 = 400.0;

/// Size of the sliding window in ticks (ms). At 125Hz mouse polling, ~8ms between
/// reports, so 24ms captures ~3 reports for smooth interpolation without adding
/// perceptible latency (well under one 60fps frame of 16.7ms extra).
const WINDOW_SIZE: usize = 24;

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

    // Main 1kHz loop with sliding window velocity
    let tick = Duration::from_micros(1000);
    let scale = BASE_SCALE * config.sensitivity;
    let y_sign = if config.invert_y { -1.0f32 } else { 1.0 };

    // Ring buffer: each slot holds (dx, dy) for one tick
    let mut ring_x = [0i32; WINDOW_SIZE];
    let mut ring_y = [0i32; WINDOW_SIZE];
    let mut ring_pos: usize = 0;
    let mut sum_x: i32 = 0;
    let mut sum_y: i32 = 0;
    let mut prev_sx: i32 = 0;
    let mut prev_sy: i32 = 0;

    // Debug
    let debug = config.debug;
    let mut dbg_tick: u32 = 0;
    let mut dbg_raw_dx: i64 = 0;
    let mut dbg_raw_dy: i64 = 0;
    let mut dbg_samples: u32 = 0;

    loop {
        let tick_start = std::time::Instant::now();

        if QUIT.load(Ordering::Relaxed) || mouse_state.quit.load(Ordering::Relaxed) {
            break;
        }

        if mouse_state.active.load(Ordering::Relaxed) {
            let (dx, dy) = mouse_state.drain();

            if debug {
                dbg_raw_dx += dx as i64;
                dbg_raw_dy += dy as i64;
                if dx != 0 || dy != 0 {
                    dbg_samples += 1;
                }
            }

            // Subtract the oldest sample, add the new one
            sum_x -= ring_x[ring_pos];
            sum_y -= ring_y[ring_pos];
            ring_x[ring_pos] = dx;
            ring_y[ring_pos] = dy;
            sum_x += dx;
            sum_y += dy;
            ring_pos = (ring_pos + 1) % WINDOW_SIZE;

            // Scale window sum directly to stick deflection
            let sx = (sum_x as f32 * scale) as i32;
            let sy = (sum_y as f32 * scale * y_sign) as i32;

            // Only emit when values actually change
            if sx != prev_sx || sy != prev_sy {
                if let Err(e) = pad.emit_stick(sx, sy) {
                    log::warn!("Failed to emit stick: {}", e);
                }
                prev_sx = sx;
                prev_sy = sy;
            }

            // Debug: print every 100 ticks (100ms)
            if debug {
                dbg_tick += 1;
                if dbg_tick >= 100 {
                    if dbg_raw_dx != 0 || dbg_raw_dy != 0 || sum_x != 0 || sum_y != 0 {
                        eprintln!(
                            "[dbg] raw({:+5},{:+5}) n={:<3} win({:+5},{:+5}) out({:+6},{:+6})",
                            dbg_raw_dx,
                            dbg_raw_dy,
                            dbg_samples,
                            sum_x,
                            sum_y,
                            sx.clamp(-32767, 32767),
                            sy.clamp(-32767, 32767),
                        );
                    }
                    dbg_tick = 0;
                    dbg_raw_dx = 0;
                    dbg_raw_dy = 0;
                    dbg_samples = 0;
                }
            }
        } else {
            // Not active — reset window and center stick
            if prev_sx != 0 || prev_sy != 0 {
                ring_x = [0; WINDOW_SIZE];
                ring_y = [0; WINDOW_SIZE];
                sum_x = 0;
                sum_y = 0;
                prev_sx = 0;
                prev_sy = 0;
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
