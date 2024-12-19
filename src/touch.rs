use anyhow::Result;
use evdev::Device;

pub struct Touch {
    device: Option<Device>,
}

impl Touch {
    pub fn new(no_touch: bool) -> Self {
        let device = if no_touch {
            None
        } else {
            Some(Device::open("/dev/input/by-path/platform-30a40000.i2c-event").unwrap())
        };

        Self { device }
    }

    pub fn wait_for_trigger(&mut self) -> Result<()> {
        let mut position_x = 0;
        let mut position_y = 0;
        loop {
            for event in self.device.as_mut().unwrap().fetch_events().unwrap() {
                if event.code() == 53 {
                    position_x = event.value();
                }
                if event.code() == 54 {
                    position_y = event.value();
                }
                if event.code() == 57 {
                    if event.value() == -1 {
                        println!("Touch release detected at ({}, {})", position_x, position_y);
                        if position_x > 1360 && position_y > 1810 {
                            println!("Touch release in target zone!");
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}
