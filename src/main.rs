use anyhow::Result;
use image::GrayImage;
use serde_json::json;
use std::{thread, time};

use clap::{Parser, Subcommand};

use resvg::render;
use resvg::tiny_skia::Pixmap;
use resvg::usvg;
use resvg::usvg::{fontdb, Options, Tree};
use std::sync::Arc;

use std::thread::sleep;
use std::time::Duration;

use evdev::{Device, EventType, InputEvent};

mod keyboard;
use crate::keyboard::Keyboard;

mod screenshot;
use crate::screenshot::Screenshot;

const INPUT_WIDTH: usize = 15725;
const INPUT_HEIGHT: usize = 20966;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sets the model to use
    #[arg(long, default_value = "gpt-4o-mini")]
    model: String,

    /// Sets the prompt to use
    #[arg(long, default_value = "default")]
    prompt: String,

    /// Do not actually submit to the model, for testing
    #[arg(short, long)]
    no_submit: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    KeyboardTest,
    TextAssist,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match &args.command {
        Some(Command::KeyboardTest) => keyboard_test(),
        Some(Command::TextAssist) => text_assistant(&args),
        None => ghostwriter(&args),
    }
}

fn keyboard_test() -> Result<()> {
    let mut keyboard = Keyboard::new();
    sleep(Duration::from_secs(1)); // Wait for device to get warmed up
                                   // let erase = "\x08".repeat(100);
                                   // let input = erase.as_str();
                                   // string_to_keypresses(&mut device, input)?;
                                   // string_to_keypresses(&mut device, "\x1b")?;
                                   // let input2 = "Hello, World! 123 @#$hidden\x08\x08\x08\n";
                                   // string_to_keypresses(&mut device, input2)?;
                                   // key_down(&mut device, Key::KEY_LEFTCTRL);
                                   // sleep(Duration::from_secs(10));
                                   // string_to_keypresses(&mut device, "4")?;
                                   // key_up(&mut device, Key::KEY_LEFTCTRL);
    keyboard.key_cmd_body()?;
    keyboard.string_to_keypresses("hmmm\n")?;
    Ok(())
}

fn text_assistant(args: &Args) -> Result<()> {
    let mut keyboard = Keyboard::new();

    loop {
        println!("Waiting for trigger (hand-touch in the upper-right corner)...");
        wait_for_trigger()?;

        keyboard.key_cmd_body()?;
        keyboard.string_to_keypresses(".")?;

        let screenshot = Screenshot::new()?;

        keyboard.string_to_keypresses(".")?;

        // Save the PNG image to a file
        screenshot.save_image("tmp/screenshot.png")?;

        keyboard.string_to_keypresses(".")?;

        let base64_image = screenshot.base64()?;

        keyboard.string_to_keypresses(".")?;

        if args.no_submit {
            println!("Image not submitted to OpenAI due to --no-submit flag");
            return Ok(());
        }

        let api_key = std::env::var("OPENAI_API_KEY")?;

        let mut body =
            serde_json::from_str::<serde_json::Value>(include_str!("../prompts/text.json"))?;
        body["model"] = json!(args.model);
        body["messages"][0]["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", base64_image)
                }
            }));

        println!("Sending request to OpenAI API...");
        keyboard.string_to_keypresses(".")?;

        let response = ureq::post("https://api.openai.com/v1/chat/completions")
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(&body);

        match response {
            Ok(response) => {
                keyboard.string_to_keypresses(".")?;
                let json: serde_json::Value = response.into_json()?;
                println!("API Response: {}", json);
                keyboard.string_to_keypresses(".")?;

                let raw_output = json["choices"][0]["message"]["content"].as_str().unwrap();
                let json_output = serde_json::from_str::<serde_json::Value>(raw_output)?;
                let input_description = json_output["input_description"].as_str().unwrap();
                let output_description = json_output["output_description"].as_str().unwrap();
                let text_data = json_output["text"].as_str().unwrap();

                println!("Input Description: {}", input_description);
                println!("Output Description: {}", output_description);
                println!("Text Data: {}", text_data);

                println!("Writing output back onto the screen");
                keyboard.key_cmd_body()?;

                // Erase the progress dots
                keyboard.string_to_keypresses("\x08\x08\x08\x08\x08\x08\x08")?;

                keyboard.string_to_keypresses(text_data)?;
                keyboard.string_to_keypresses("\n\n")?;
            }
            Err(ureq::Error::Status(code, response)) => {
                println!("HTTP Error: {} {}", code, response.status_text());
                if let Ok(json) = response.into_json::<serde_json::Value>() {
                    println!("Error details: {}", json);
                } else {
                    println!("Failed to parse error response as JSON");
                }
                return Err(anyhow::anyhow!("API request failed"));
            }
            Err(e) => return Err(anyhow::anyhow!("Request failed: {}", e)),
        }
    }

    Ok(())
}

fn ghostwriter(args: &Args) -> Result<()> {
    // Open the device for drawing
    let mut device = Device::open("/dev/input/event1")?;

    loop {
        println!("Waiting for trigger (hand-touch in the upper-right corner)...");
        wait_for_trigger()?;

        let screenshot = Screenshot::new()?;

        draw_line(
            &mut device,
            screen_to_input((1340, 5)),
            screen_to_input((1390, 75)),
        )?;

        // Save the PNG image to a file
        screenshot.save_image("tmp/screenshot.png")?;

        let base64_image = screenshot.base64()?;

        if args.no_submit {
            println!("Image not submitted to OpenAI due to --no-submit flag");
            return Ok(());
        }

        let api_key = std::env::var("OPENAI_API_KEY")?;

        // Get the base prompt from prompts/base.json as a serde json object
        // Then modify it to set our current model and add the image
        let mut body =
            serde_json::from_str::<serde_json::Value>(include_str!("../prompts/base.json"))?;
        body["model"] = json!(args.model);
        // body["model"] = json!("gpt-4o");
        // body["model"] = json!("o1-preview");
        body["messages"][0]["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", base64_image)
                }
            }));

        println!("Sending request to OpenAI API...");
        draw_line(
            &mut device,
            screen_to_input((1340, 75)),
            screen_to_input((1390, 5)),
        )?;

        let response = ureq::post("https://api.openai.com/v1/chat/completions")
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(&body);

        match response {
            Ok(response) => {
                let json: serde_json::Value = response.into_json()?;
                println!("API Response: {}", json);
                draw_line(
                    &mut device,
                    screen_to_input((1365, 5)),
                    screen_to_input((1365, 75)),
                )?;

                let raw_output = json["choices"][0]["message"]["content"].as_str().unwrap();
                let json_output = serde_json::from_str::<serde_json::Value>(raw_output)?;
                let input_description = json_output["input_description"].as_str().unwrap();
                let output_description = json_output["output_description"].as_str().unwrap();
                let svg_data = json_output["svg"].as_str().unwrap();

                println!("Input Description: {}", input_description);
                println!("Output Description: {}", output_description);
                println!("SVG Data: {}", svg_data);

                println!("Rendering SVG to bitmap");
                let bitmap =
                    svg_to_bitmap(svg_data, REMARKABLE_WIDTH as u32, REMARKABLE_HEIGHT as u32)?;
                write_bitmap_to_file(&bitmap, "tmp/debug_bitmap.png")?;

                println!("Drawing output back onto the screen");
                let mut is_pen_down = false;
                for (y, row) in bitmap.iter().enumerate() {
                    for (x, &pixel) in row.iter().enumerate() {
                        if pixel {
                            if !is_pen_down {
                                draw_goto_xy(&mut device, screen_to_input((x as i32, y as i32)))?;
                                draw_pen_down(&mut device)?;
                                is_pen_down = true;
                                thread::sleep(time::Duration::from_millis(1));
                            }
                            draw_goto_xy(&mut device, screen_to_input((x as i32, y as i32)))?;
                            draw_goto_xy(&mut device, screen_to_input((x as i32 + 1, y as i32)))?;
                            // draw_goto_xy(&mut device, screen_to_input((x as i32 + 2, y as i32)))?;
                        } else {
                            if is_pen_down {
                                draw_pen_up(&mut device)?;
                                is_pen_down = false;
                                thread::sleep(time::Duration::from_millis(1));
                            }
                        }
                    }

                    // At the end of the row, pick up the pen no matter what
                    draw_pen_up(&mut device)?;
                    is_pen_down = false;
                    thread::sleep(time::Duration::from_millis(5));
                }

                draw_line(
                    &mut device,
                    screen_to_input((1330, 40)),
                    screen_to_input((1390, 40)),
                )?;
            }
            Err(ureq::Error::Status(code, response)) => {
                println!("HTTP Error: {} {}", code, response.status_text());
                if let Ok(json) = response.into_json::<serde_json::Value>() {
                    println!("Error details: {}", json);
                } else {
                    println!("Failed to parse error response as JSON");
                }
                return Err(anyhow::anyhow!("API request failed"));
            }
            Err(e) => return Err(anyhow::anyhow!("Request failed: {}", e)),
        }
    }
    Ok(())
}

use image;

fn draw_line(device: &mut Device, (x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> Result<()> {
    // println!("Drawing from ({}, {}) to ({}, {})", x1, y1, x2, y2);

    // We know this is a straight line
    // So figure out the length
    // Then divide it into enough steps to only go 10 units or so
    // Start at x1, y1
    // And then for each step add the right amount to x and y

    let length = ((x2 as f32 - x1 as f32).powf(2.0) + (y2 as f32 - y1 as f32).powf(2.0)).sqrt();
    // 5.0 is the maximum distance between points
    // If this is too small
    let steps = (length / 5.0).ceil() as i32;
    let dx = (x2 - x1) / steps;
    let dy = (y2 - y1) / steps;
    // println!(
    //     "Drawing from ({}, {}) to ({}, {}) in {} steps",
    //     x1, y1, x2, y2, steps
    // );

    draw_pen_up(device)?;
    draw_goto_xy(device, (x1, y1))?;
    draw_pen_down(device)?;

    for i in 0..steps {
        let x = x1 + dx * i;
        let y = y1 + dy * i;
        draw_goto_xy(device, (x, y))?;
        // println!("Drawing to point at ({}, {})", x, y);
    }

    draw_pen_up(device)?;

    Ok(())
}

// fn draw_dot(device: &mut Device, (x, y): (i32, i32)) -> Result<()> {
//     // println!("Drawing at ({}, {})", x, y);
//     draw_goto_xy(device, (x, y))?;
//     draw_pen_down(device)?;
//
//     // Wiggle a little bit
//     for n in 0..2 {
//         draw_goto_xy(device, (x + n, y + n))?;
//     }
//
//     draw_pen_up(device)?;
//
//     // sleep for 5ms
//     thread::sleep(time::Duration::from_millis(1));
//
//     Ok(())
// }

fn draw_pen_down(device: &mut Device) -> Result<()> {
    device.send_events(&[
        InputEvent::new(EventType::KEY, 320, 1), // BTN_TOOL_PEN
        InputEvent::new(EventType::KEY, 330, 1), // BTN_TOUCH
        InputEvent::new(EventType::ABSOLUTE, 24, 2630), // ABS_PRESSURE (max pressure)
        InputEvent::new(EventType::ABSOLUTE, 25, 0), // ABS_DISTANCE
        InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
    ])?;
    Ok(())
}

fn draw_pen_up(device: &mut Device) -> Result<()> {
    device.send_events(&[
        InputEvent::new(EventType::ABSOLUTE, 24, 0), // ABS_PRESSURE
        InputEvent::new(EventType::ABSOLUTE, 25, 100), // ABS_DISTANCE
        InputEvent::new(EventType::KEY, 330, 0),     // BTN_TOUCH
        InputEvent::new(EventType::KEY, 320, 0),     // BTN_TOOL_PEN
        InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
    ])?;
    Ok(())
}

fn draw_goto_xy(device: &mut Device, (x, y): (i32, i32)) -> Result<()> {
    // println!("Drawing to point at ({}, {})", x, y);
    device.send_events(&[
        InputEvent::new(EventType::ABSOLUTE, 0, x),        // ABS_X
        InputEvent::new(EventType::ABSOLUTE, 1, y),        // ABS_Y
        InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
    ])?;
    Ok(())
}

fn screen_to_input((x, y): (i32, i32)) -> (i32, i32) {
    // Swap and normalize the coordinates
    let x_normalized = x as f32 / REMARKABLE_WIDTH as f32;
    let y_normalized = y as f32 / REMARKABLE_HEIGHT as f32;

    let x_input = ((1.0 - y_normalized) * INPUT_HEIGHT as f32) as i32;
    let y_input = (x_normalized * INPUT_WIDTH as f32) as i32;
    (x_input, y_input)
}

fn svg_to_bitmap(svg_data: &str, width: u32, height: u32) -> Result<Vec<Vec<bool>>> {
    let mut opt = Options::default();
    let mut fontdb = fontdb::Database::new();
    fontdb.load_fonts_dir("/usr/share/fonts/ttf/noto");
    opt.fontdb = Arc::new(fontdb);

    let tree = match Tree::from_str(svg_data, &opt) {
        Ok(tree) => tree,
        Err(e) => {
            println!("Error parsing SVG: {}. Using fallback SVG.", e);
            let fallback_svg = r#"<svg width='1404' height='1872' xmlns='http://www.w3.org/2000/svg'><text x='300' y='1285' font-family='Noto Sans' font-size='24'>ERROR!</text></svg>"#;
            Tree::from_str(fallback_svg, &opt)?
        }
    };

    let mut pixmap = Pixmap::new(width, height).unwrap();
    render(&tree, usvg::Transform::default(), &mut pixmap.as_mut());

    let bitmap = pixmap
        .pixels()
        .chunks(width as usize)
        .map(|row| row.iter().map(|p| p.alpha() > 128).collect())
        .collect();

    Ok(bitmap)
}

fn write_bitmap_to_file(bitmap: &Vec<Vec<bool>>, filename: &str) -> Result<()> {
    let width = bitmap[0].len();
    let height = bitmap.len();
    let mut img = GrayImage::new(width as u32, height as u32);

    for (y, row) in bitmap.iter().enumerate() {
        for (x, &pixel) in row.iter().enumerate() {
            img.put_pixel(
                x as u32,
                y as u32,
                image::Luma([if pixel { 0 } else { 255 }]),
            );
        }
    }

    img.save(filename)?;
    println!("Bitmap saved to {}", filename);
    Ok(())
}

fn wait_for_trigger() -> Result<()> {
    let mut device = Device::open("/dev/input/event2")?; // Touch input device
    let mut position_x = 0;
    let mut position_y = 0;
    loop {
        for event in device.fetch_events().unwrap() {
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

// use evdev::{uinput::VirtualDevice, uinput::VirtualDeviceBuilder, AttributeSet, Key};

// use std::collections::HashMap;

// use crate::keyboard::Keyboard;
