## Description

Linux mouse-to-joystick injector for RetroArch that grabs your mouse via evdev and maps it to a virtual gamepad stick via uinput. Works with any game on any core, on Wayland or X11.

## Skills / Tools / Stack

- Rust
- Linux evdev / uinput
- Unix signals (SIGUSR1)
- clap CLI framework

# Summary

m2joy reads raw mouse input from `/dev/input/eventX`, creates a virtual gamepad ("m2joy Stick") via `/dev/uinput`, and converts mouse velocity to analog stick deflection at 1kHz using a leaky accumulator.

Control is fully command-based. Run `m2joy toggle` from another process to grab or ungrab the mouse—designed for Hyprland, sway, or any window manager keybind. Under the hood, toggle sends a SIGUSR1 signal to the running instance. No keyboard device access required.

The decay parameter controls how quickly the stick returns to center after you stop moving. Lower values feel snappy, higher values feel smooth. Sensitivity scales the raw mouse input before it hits the accumulator.

## Features

- 1kHz polling loop with leaky accumulator for smooth analog stick output
- Virtual gamepad via uinput recognized by RetroArch as a standard controller
- Command-based toggle with `m2joy toggle` and `m2joy quit`
- SIGUSR1 signal toggle for window manager keybind integration
- Auto-detection of mouse device from `/dev/input/event*`
- Configurable sensitivity, decay, Y-axis inversion, and stick output
- Left or right stick output selection
- Evdev grab/ungrab to capture and release the mouse

### Roadmap

1. Add per-game config profiles
2. Implement mouse button to gamepad button mapping
3. Build acceleration curves for non-linear sensitivity
4. Add support for multiple mice
5. Create a status indicator via desktop notification

### Instructions

1. Add yourself to the input group with `sudo usermod -aG input $USER` and re-login
2. Ensure uinput is loaded with `sudo modprobe uinput`
3. Build with `cargo build --release`
4. Run `./target/release/m2joy` to start the daemon
5. Toggle grab with `m2joy toggle` from another terminal or keybind
6. In RetroArch go to Settings > Input > Port 1 Controls > Device Index and select m2joy Stick

#### Hyprland

```
bind = SUPER, F9, exec, m2joy toggle
```

#### sway / i3

```
bindsym $mod+F9 exec m2joy toggle
```

#### Options

| Option | Default | Description |
|---|---|---|
| `-s, --sensitivity` | 1.0 | Mouse sensitivity multiplier |
| `--invert-y` | off | Invert Y axis |
| `-d, --device` | auto | Specific evdev path (e.g. `/dev/input/event5`) |
| `--left-stick` | off | Output to left stick instead of right |
| `--decay` | 0.95 | Smoothing factor (0.90=snappy, 0.99=smooth) |

### License

MIT
