use anyhow::Result;
use evdev::Device;

pub struct Touch {
    device: Device,
}

impl Touch {
    pub fn new() -> Self {
        let device = Device::open("/dev/input/event2").unwrap();

        Self { device: device }
    }

    pub fn wait_for_trigger(&mut self) -> Result<()> {
        let mut position_x = 0;
        let mut position_y = 0;
        loop {
            for event in self.device.fetch_events().unwrap() {
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
