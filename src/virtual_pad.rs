use evdev::uinput::VirtualDeviceBuilder;
use evdev::{AbsInfo, AbsoluteAxisType, AttributeSet, BusType, InputId, Key, UinputAbsSetup};

const STICK_MIN: i32 = -32767;
const STICK_MAX: i32 = 32767;

pub struct VirtualPad {
    device: evdev::uinput::VirtualDevice,
    axis_x: AbsoluteAxisType,
    axis_y: AbsoluteAxisType,
}

impl VirtualPad {
    pub fn new(use_left_stick: bool) -> std::io::Result<Self> {
        let abs = |axis: AbsoluteAxisType| -> UinputAbsSetup {
            UinputAbsSetup::new(axis, AbsInfo::new(0, STICK_MIN, STICK_MAX, 16, 128, 1))
        };

        // Declare a few buttons so RetroArch classifies this as a gamepad
        let mut keys = AttributeSet::<Key>::new();
        keys.insert(Key::BTN_SOUTH);
        keys.insert(Key::BTN_EAST);
        keys.insert(Key::BTN_NORTH);
        keys.insert(Key::BTN_WEST);

        let device = VirtualDeviceBuilder::new()?
            .name("m2joy Stick")
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x1234, 0x5678, 1))
            .with_keys(&keys)?
            .with_absolute_axis(&abs(AbsoluteAxisType::ABS_X))?
            .with_absolute_axis(&abs(AbsoluteAxisType::ABS_Y))?
            .with_absolute_axis(&abs(AbsoluteAxisType::ABS_RX))?
            .with_absolute_axis(&abs(AbsoluteAxisType::ABS_RY))?
            .build()?;

        let (axis_x, axis_y) = if use_left_stick {
            (AbsoluteAxisType::ABS_X, AbsoluteAxisType::ABS_Y)
        } else {
            (AbsoluteAxisType::ABS_RX, AbsoluteAxisType::ABS_RY)
        };

        log::info!(
            "Created virtual gamepad (output: {} stick)",
            if use_left_stick { "left" } else { "right" }
        );

        Ok(Self {
            device,
            axis_x,
            axis_y,
        })
    }

    pub fn emit_stick(&mut self, x: i32, y: i32) -> std::io::Result<()> {
        let x = x.clamp(STICK_MIN, STICK_MAX);
        let y = y.clamp(STICK_MIN, STICK_MAX);
        self.device.emit(&[
            evdev::InputEvent::new_now(evdev::EventType::ABSOLUTE, self.axis_x.0, x),
            evdev::InputEvent::new_now(evdev::EventType::ABSOLUTE, self.axis_y.0, y),
            evdev::InputEvent::new_now(evdev::EventType::SYNCHRONIZATION, 0, 0),
        ])
    }
}
