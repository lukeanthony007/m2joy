# m2joy

Linux mouse-to-joystick injector for RetroArch. Grabs your mouse via evdev and maps it to a virtual gamepad stick via uinput. Works with any game on any core, on Wayland or X11.

## How it works

1. Reads raw mouse input from `/dev/input/eventX`
2. Creates a virtual gamepad ("m2joy Stick") via `/dev/uinput`
3. Converts mouse velocity to analog stick deflection at 1kHz using a leaky accumulator
4. Toggle with `m2joy toggle` to grab/ungrab, quit with `m2joy quit`

## Build

```
cargo build --release
```

Binary is at `target/release/m2joy`.

## Permissions

You need access to evdev and uinput devices:

```
sudo usermod -aG input $USER
```

Then log out and back in. If `/dev/uinput` doesn't exist:

```
sudo modprobe uinput
```

## Usage

```
m2joy [OPTIONS]
```

| Option | Default | Description |
|---|---|---|
| `-s, --sensitivity` | 1.0 | Mouse sensitivity multiplier |
| `--invert-y` | off | Invert Y axis |
| `-d, --device` | auto | Specific evdev path (e.g. `/dev/input/event5`) |
| `--left-stick` | off | Output to left stick instead of right |
| `--decay` | 0.95 | Smoothing factor (0.90=snappy, 0.99=smooth) |

### Examples

```bash
# Default — right stick, sensitivity 1.0
m2joy

# Double sensitivity, inverted Y
m2joy -s 2.0 --invert-y

# Output to left stick (for games that use left analog for aiming)
m2joy --left-stick

# Smoother output (slightly more latency)
m2joy --decay 0.98

# Specific mouse device
m2joy -d /dev/input/event5
```

## Remote control

While m2joy is running, you can control it from another terminal or a window manager keybind:

```bash
m2joy toggle   # grab/ungrab the mouse
m2joy quit     # shut down cleanly
```

### Hyprland

Add to `~/.config/hypr/hyprland.conf`:

```
bind = SUPER, F9, exec, m2joy toggle
```

### sway / i3

```
bindsym $mod+F9 exec m2joy toggle
```

## RetroArch setup

1. Launch `m2joy`, then launch RetroArch
2. Go to **Settings > Input > Port 1 Controls > Device Index**
3. Select **m2joy Stick** from the device list (not Device Type — that's for the emulated controller)
4. Map the stick axes to whatever your game uses for camera/aiming:
   - **N64 FPS games**: map right stick to C-buttons
   - **PS1/PS2/GameCube**: right stick usually maps directly
   - **SNES/Genesis**: use left stick mode (`--left-stick`)

You only need to configure this once per core — RetroArch saves per-core input remaps.

## Tuning

The `--decay` parameter controls how the leaky accumulator converts mouse velocity to stick position:

- **Lower values (0.90)**: snappy, stick returns to center quickly when you stop moving. Can feel jittery at low DPI.
- **Higher values (0.98-0.99)**: smoother output, but the stick takes longer to return to center after you stop moving (~100-200ms).
- **Default (0.95)**: ~60ms return to center. Good starting point.

If aiming feels too slow or too fast, adjust `--sensitivity` first. If it feels jittery or laggy, adjust `--decay`.

