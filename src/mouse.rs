use evdev::{Device, InputEventKind, RelativeAxisType};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

pub struct MouseState {
    pub dx: AtomicI32,
    pub dy: AtomicI32,
    pub active: AtomicBool,
    pub quit: AtomicBool,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            dx: AtomicI32::new(0),
            dy: AtomicI32::new(0),
            active: AtomicBool::new(false),
            quit: AtomicBool::new(false),
        }
    }

    /// Drain accumulated deltas, returning (dx, dy) and resetting to zero.
    pub fn drain(&self) -> (i32, i32) {
        let dx = self.dx.swap(0, Ordering::Relaxed);
        let dy = self.dy.swap(0, Ordering::Relaxed);
        (dx, dy)
    }
}

/// Find a mouse device by enumerating /dev/input/event*.
/// Returns the first device that supports REL_X, REL_Y, and BTN_LEFT.
pub fn find_mouse_device() -> Option<PathBuf> {
    use evdev::Key;
    for i in 0..64 {
        let path = PathBuf::from(format!("/dev/input/event{}", i));
        if !path.exists() {
            continue;
        }
        if let Ok(device) = Device::open(&path) {
            let has_rel_x = device
                .supported_relative_axes()
                .is_some_and(|axes| axes.contains(RelativeAxisType::REL_X));
            let has_rel_y = device
                .supported_relative_axes()
                .is_some_and(|axes| axes.contains(RelativeAxisType::REL_Y));
            let has_btn_left = device
                .supported_keys()
                .is_some_and(|keys| keys.contains(Key::BTN_LEFT));

            if has_rel_x && has_rel_y && has_btn_left {
                log::info!(
                    "Found mouse: {} at {}",
                    device.name().unwrap_or("unknown"),
                    path.display()
                );
                return Some(path);
            }
        }
    }
    None
}

pub struct MouseReader {
    device: Device,
    state: Arc<MouseState>,
}

impl MouseReader {
    pub fn new(device_path: &str, state: Arc<MouseState>) -> std::io::Result<Self> {
        let device = Device::open(device_path)?;
        log::info!(
            "Opened mouse device: {} ({})",
            device.name().unwrap_or("unknown"),
            device_path
        );
        Ok(Self { device, state })
    }

    /// Run the blocking event loop. Call from a dedicated thread.
    pub fn run(&mut self) {
        loop {
            if self.state.quit.load(Ordering::Relaxed) {
                break;
            }

            // Check for external toggle signal (SIGUSR1 via `m2joy toggle`)
            if crate::TOGGLE
                .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                let was_active = self.state.active.load(Ordering::Relaxed);
                if was_active {
                    self.state.active.store(false, Ordering::Relaxed);
                    if let Err(e) = self.device.ungrab() {
                        log::warn!("Failed to ungrab mouse: {}", e);
                    }
                    log::info!("Mouse released");
                } else {
                    if let Err(e) = self.device.grab() {
                        log::warn!("Failed to grab mouse: {}", e);
                    }
                    self.state.active.store(true, Ordering::Relaxed);
                    log::info!("Mouse grabbed");
                }
            }

            let events: Vec<_> = match self.device.fetch_events() {
                Ok(iter) => iter.collect(),
                Err(e) => {
                    if self.state.quit.load(Ordering::Relaxed) {
                        break;
                    }
                    // SIGUSR1 interrupts the blocking read with EINTR — just loop
                    // back and check the TOGGLE flag above.
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    log::error!("Error reading mouse events: {}", e);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
            };

            for ev in &events {
                if let InputEventKind::RelAxis(axis) = ev.kind() {
                    if !self.state.active.load(Ordering::Relaxed) {
                        continue;
                    }
                    match axis {
                        RelativeAxisType::REL_X => {
                            self.state.dx.fetch_add(ev.value(), Ordering::Relaxed);
                        }
                        RelativeAxisType::REL_Y => {
                            self.state.dy.fetch_add(ev.value(), Ordering::Relaxed);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Ungrab on exit
        let _ = self.device.ungrab();
    }
}
