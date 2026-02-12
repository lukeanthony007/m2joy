use clap::Parser;

/// Linux mouse-to-joystick injector for RetroArch (Wayland/evdev).
/// Grabs your mouse and maps it to a virtual gamepad stick.
#[derive(Parser, Debug)]
#[command(name = "m2joy")]
pub struct Config {
    /// Mouse sensitivity multiplier
    #[arg(short, long, default_value_t = 1.0)]
    pub sensitivity: f32,

    /// Invert Y axis
    #[arg(long, default_value_t = false)]
    pub invert_y: bool,

    /// Specific evdev device path (e.g. /dev/input/event5)
    #[arg(short, long)]
    pub device: Option<String>,

    /// Output to left stick (ABS_X/ABS_Y) instead of right stick (ABS_RX/ABS_RY)
    #[arg(long, default_value_t = false)]
    pub left_stick: bool,

    /// Smoothing decay factor per ms (0.90-0.99, higher = smoother but laggier)
    #[arg(long, default_value_t = 0.95)]
    pub decay: f32,

    /// Print debug diagnostics every 100ms (raw deltas, accumulator, output)
    #[arg(long, default_value_t = false)]
    pub debug: bool,
}
