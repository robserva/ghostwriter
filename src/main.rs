use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use image::GrayImage;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::io::{Read, Seek};
use std::{thread, time};

use clap::{Parser, Subcommand};

use resvg::render;
use resvg::tiny_skia::Pixmap;
use resvg::usvg;
use resvg::usvg::{fontdb, Options, Tree};
use std::sync::Arc;

use std::process; // ::Command;

use evdev::{Device, EventType, InputEvent};

const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;
const BYTES_PER_PIXEL: usize = 2;
const WINDOW_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;
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
        keyboard.key_cmd_body()?;
        keyboard.string_to_keypresses(".")?;

        let screenshot_data = take_screenshot()?;
        keyboard.string_to_keypresses(".")?;

        // Save the PNG image to a file
        let png_filename = "tmp/screenshot.png";
        let mut png_file = File::create(png_filename)?;
        png_file.write_all(&screenshot_data)?;
        println!("PNG image saved to {}", png_filename);
        keyboard.string_to_keypresses(".")?;

        let base64_image = general_purpose::STANDARD.encode(&screenshot_data);

        // Save the base64 encoded image to a file
        let base64_filename = "tmp/screenshot_base64.txt";
        let mut base64_file = File::create(base64_filename)?;
        base64_file.write_all(base64_image.as_bytes())?;
        println!("Base64 encoded image saved to {}", base64_filename);
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

        let screenshot_data = take_screenshot()?;

        draw_line(
            &mut device,
            screen_to_input((1340, 5)),
            screen_to_input((1390, 75)),
        )?;

        // Save the PNG image to a file
        let png_filename = "tmp/screenshot.png";
        let mut png_file = File::create(png_filename)?;
        png_file.write_all(&screenshot_data)?;
        println!("PNG image saved to {}", png_filename);

        let base64_image = general_purpose::STANDARD.encode(&screenshot_data);

        // Save the base64 encoded image to a file
        let base64_filename = "tmp/screenshot_base64.txt";
        let mut base64_file = File::create(base64_filename)?;
        base64_file.write_all(base64_image.as_bytes())?;
        println!("Base64 encoded image saved to {}", base64_filename);

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

fn take_screenshot() -> Result<Vec<u8>> {
    // Find xochitl's process
    let pid = find_xochitl_pid()?;

    // Find framebuffer location in memory
    let skip_bytes = find_framebuffer_address(&pid)?;

    // Read the framebuffer data
    let screenshot_data = read_framebuffer(&pid, skip_bytes)?;

    // Process the image data (transpose, color correction, etc.)
    let processed_data = process_image(screenshot_data)?;

    Ok(processed_data)
}

fn find_xochitl_pid() -> Result<String> {
    let output = process::Command::new("pidof").arg("xochitl").output()?;
    let pids = String::from_utf8(output.stdout)?;
    for pid in pids.split_whitespace() {
        let has_fb = process::Command::new("grep")
            .args(&["-C1", "/dev/fb0", &format!("/proc/{}/maps", pid)])
            .output()?;
        if !has_fb.stdout.is_empty() {
            return Ok(pid.to_string());
        }
    }
    anyhow::bail!("No xochitl process with /dev/fb0 found")
}

fn find_framebuffer_address(pid: &str) -> Result<u64> {
    let output = process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "grep -C1 '/dev/fb0' /proc/{}/maps | tail -n1 | sed 's/-.*$//'",
            pid
        ))
        .output()?;
    let address_hex = String::from_utf8(output.stdout)?.trim().to_string();
    let address = u64::from_str_radix(&address_hex, 16)?;
    Ok(address + 7)
}

fn read_framebuffer(pid: &str, skip_bytes: u64) -> Result<Vec<u8>> {
    let mut buffer = vec![0u8; WINDOW_BYTES];
    let mut file = std::fs::File::open(format!("/proc/{}/mem", pid))?;
    file.seek(std::io::SeekFrom::Start(skip_bytes))?;
    file.read_exact(&mut buffer)?;
    Ok(buffer)
}

fn process_image(data: Vec<u8>) -> Result<Vec<u8>> {
    // Implement image processing here (transpose, color correction, etc.)
    // For now, we'll just encode the raw data to PNG
    encode_png(&data)
}

use image;

fn encode_png(raw_data: &[u8]) -> Result<Vec<u8>> {
    let raw_u8: Vec<u8> = raw_data
        .chunks_exact(2)
        .map(|chunk| u8::from_le_bytes([chunk[1]]))
        .collect();

    let mut processed = vec![0u8; (REMARKABLE_WIDTH * REMARKABLE_HEIGHT) as usize];

    for y in 0..REMARKABLE_HEIGHT {
        for x in 0..REMARKABLE_WIDTH {
            let src_idx =
                (REMARKABLE_HEIGHT - 1 - y) + (REMARKABLE_WIDTH - 1 - x) * REMARKABLE_HEIGHT;
            let dst_idx = y * REMARKABLE_WIDTH + x;
            processed[dst_idx as usize] = apply_curves(raw_u8[src_idx as usize]);
        }
    }

    let img = GrayImage::from_raw(REMARKABLE_WIDTH as u32, REMARKABLE_HEIGHT as u32, processed)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image from raw data"))?;

    let mut png_data = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
    encoder.encode(
        img.as_raw(),
        REMARKABLE_WIDTH as u32,
        REMARKABLE_HEIGHT as u32,
        image::ColorType::L8,
    )?;

    Ok(png_data)
}

fn apply_curves(value: u8) -> u8 {
    let normalized = value as f32 / 255.0;
    let adjusted = if normalized < 0.045 {
        0.0
    } else if normalized < 0.06 {
        (normalized - 0.045) / (0.06 - 0.045)
    } else {
        1.0
    };
    (adjusted * 255.0) as u8
}

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

fn draw_dot(device: &mut Device, (x, y): (i32, i32)) -> Result<()> {
    // println!("Drawing at ({}, {})", x, y);
    draw_goto_xy(device, (x, y))?;
    draw_pen_down(device)?;

    // Wiggle a little bit
    for n in 0..2 {
        draw_goto_xy(device, (x + n, y + n))?;
    }

    draw_pen_up(device)?;

    // sleep for 5ms
    thread::sleep(time::Duration::from_millis(1));

    Ok(())
}

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

use evdev::{uinput::VirtualDevice, uinput::VirtualDeviceBuilder, AttributeSet, Key};
use std::thread::sleep;
use std::time::Duration;

use std::collections::HashMap;


pub struct Keyboard {
    device: VirtualDevice,
    key_map: HashMap<char, (Key, bool)>,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            device: Self::create_virtual_device(),
            key_map: Self::create_key_map(),
        }
    }

    fn create_virtual_device() -> VirtualDevice {
        let mut keys = AttributeSet::new();

        keys.insert(Key::KEY_A);
        keys.insert(Key::KEY_B);
        keys.insert(Key::KEY_C);
        keys.insert(Key::KEY_D);
        keys.insert(Key::KEY_E);
        keys.insert(Key::KEY_F);
        keys.insert(Key::KEY_G);
        keys.insert(Key::KEY_H);
        keys.insert(Key::KEY_I);
        keys.insert(Key::KEY_J);
        keys.insert(Key::KEY_K);
        keys.insert(Key::KEY_L);
        keys.insert(Key::KEY_M);
        keys.insert(Key::KEY_N);
        keys.insert(Key::KEY_O);
        keys.insert(Key::KEY_P);
        keys.insert(Key::KEY_Q);
        keys.insert(Key::KEY_R);
        keys.insert(Key::KEY_S);
        keys.insert(Key::KEY_T);
        keys.insert(Key::KEY_U);
        keys.insert(Key::KEY_V);
        keys.insert(Key::KEY_W);
        keys.insert(Key::KEY_X);
        keys.insert(Key::KEY_Y);
        keys.insert(Key::KEY_Z);

        keys.insert(Key::KEY_1);
        keys.insert(Key::KEY_2);
        keys.insert(Key::KEY_3);
        keys.insert(Key::KEY_4);
        keys.insert(Key::KEY_5);
        keys.insert(Key::KEY_6);
        keys.insert(Key::KEY_7);
        keys.insert(Key::KEY_8);
        keys.insert(Key::KEY_9);
        keys.insert(Key::KEY_0);

        // Add punctuation and special keys
        keys.insert(Key::KEY_SPACE);
        keys.insert(Key::KEY_ENTER);
        keys.insert(Key::KEY_TAB);
        keys.insert(Key::KEY_LEFTSHIFT);
        keys.insert(Key::KEY_MINUS);
        keys.insert(Key::KEY_EQUAL);
        keys.insert(Key::KEY_LEFTBRACE);
        keys.insert(Key::KEY_RIGHTBRACE);
        keys.insert(Key::KEY_BACKSLASH);
        keys.insert(Key::KEY_SEMICOLON);
        keys.insert(Key::KEY_APOSTROPHE);
        keys.insert(Key::KEY_GRAVE);
        keys.insert(Key::KEY_COMMA);
        keys.insert(Key::KEY_DOT);
        keys.insert(Key::KEY_SLASH);

        keys.insert(Key::KEY_BACKSPACE);
        keys.insert(Key::KEY_ESC);

        keys.insert(Key::KEY_LEFTCTRL);
        keys.insert(Key::KEY_LEFTALT);

        VirtualDeviceBuilder::new()
            .unwrap()
            .name("Virtual Keyboard")
            .with_keys(&keys)
            .unwrap()
            .build()
            .unwrap()
    }

    fn create_key_map() -> HashMap<char, (Key, bool)> {
        let mut key_map = HashMap::new();

        // Lowercase letters
        key_map.insert('a', (Key::KEY_A, false));
        key_map.insert('b', (Key::KEY_B, false));
        key_map.insert('c', (Key::KEY_C, false));
        key_map.insert('d', (Key::KEY_D, false));
        key_map.insert('e', (Key::KEY_E, false));
        key_map.insert('f', (Key::KEY_F, false));
        key_map.insert('g', (Key::KEY_G, false));
        key_map.insert('h', (Key::KEY_H, false));
        key_map.insert('i', (Key::KEY_I, false));
        key_map.insert('j', (Key::KEY_J, false));
        key_map.insert('k', (Key::KEY_K, false));
        key_map.insert('l', (Key::KEY_L, false));
        key_map.insert('m', (Key::KEY_M, false));
        key_map.insert('n', (Key::KEY_N, false));
        key_map.insert('o', (Key::KEY_O, false));
        key_map.insert('p', (Key::KEY_P, false));
        key_map.insert('q', (Key::KEY_Q, false));
        key_map.insert('r', (Key::KEY_R, false));
        key_map.insert('s', (Key::KEY_S, false));
        key_map.insert('t', (Key::KEY_T, false));
        key_map.insert('u', (Key::KEY_U, false));
        key_map.insert('v', (Key::KEY_V, false));
        key_map.insert('w', (Key::KEY_W, false));
        key_map.insert('x', (Key::KEY_X, false));
        key_map.insert('y', (Key::KEY_Y, false));
        key_map.insert('z', (Key::KEY_Z, false));

        // Uppercase letters
        key_map.insert('A', (Key::KEY_A, true));
        key_map.insert('B', (Key::KEY_B, true));
        key_map.insert('C', (Key::KEY_C, true));
        key_map.insert('D', (Key::KEY_D, true));
        key_map.insert('E', (Key::KEY_E, true));
        key_map.insert('F', (Key::KEY_F, true));
        key_map.insert('G', (Key::KEY_G, true));
        key_map.insert('H', (Key::KEY_H, true));
        key_map.insert('I', (Key::KEY_I, true));
        key_map.insert('J', (Key::KEY_J, true));
        key_map.insert('K', (Key::KEY_K, true));
        key_map.insert('L', (Key::KEY_L, true));
        key_map.insert('M', (Key::KEY_M, true));
        key_map.insert('N', (Key::KEY_N, true));
        key_map.insert('O', (Key::KEY_O, true));
        key_map.insert('P', (Key::KEY_P, true));
        key_map.insert('Q', (Key::KEY_Q, true));
        key_map.insert('R', (Key::KEY_R, true));
        key_map.insert('S', (Key::KEY_S, true));
        key_map.insert('T', (Key::KEY_T, true));
        key_map.insert('U', (Key::KEY_U, true));
        key_map.insert('V', (Key::KEY_V, true));
        key_map.insert('W', (Key::KEY_W, true));
        key_map.insert('X', (Key::KEY_X, true));
        key_map.insert('Y', (Key::KEY_Y, true));
        key_map.insert('Z', (Key::KEY_Z, true));

        // Numbers
        key_map.insert('0', (Key::KEY_0, false));
        key_map.insert('1', (Key::KEY_1, false));
        key_map.insert('2', (Key::KEY_2, false));
        key_map.insert('3', (Key::KEY_3, false));
        key_map.insert('4', (Key::KEY_4, false));
        key_map.insert('5', (Key::KEY_5, false));
        key_map.insert('6', (Key::KEY_6, false));
        key_map.insert('7', (Key::KEY_7, false));
        key_map.insert('8', (Key::KEY_8, false));
        key_map.insert('9', (Key::KEY_9, false));

        // Special characters
        key_map.insert('!', (Key::KEY_1, true));
        key_map.insert('@', (Key::KEY_2, true));
        key_map.insert('#', (Key::KEY_3, true));
        key_map.insert('$', (Key::KEY_4, true));
        key_map.insert('%', (Key::KEY_5, true));
        key_map.insert('^', (Key::KEY_6, true));
        key_map.insert('&', (Key::KEY_7, true));
        key_map.insert('*', (Key::KEY_8, true));
        key_map.insert('(', (Key::KEY_9, true));
        key_map.insert(')', (Key::KEY_0, true));
        key_map.insert('_', (Key::KEY_MINUS, true));
        key_map.insert('+', (Key::KEY_EQUAL, true));
        key_map.insert('{', (Key::KEY_LEFTBRACE, true));
        key_map.insert('}', (Key::KEY_RIGHTBRACE, true));
        key_map.insert('|', (Key::KEY_BACKSLASH, true));
        key_map.insert(':', (Key::KEY_SEMICOLON, true));
        key_map.insert('"', (Key::KEY_APOSTROPHE, true));
        key_map.insert('<', (Key::KEY_COMMA, true));
        key_map.insert('>', (Key::KEY_DOT, true));
        key_map.insert('?', (Key::KEY_SLASH, true));
        key_map.insert('~', (Key::KEY_GRAVE, true));

        // Common punctuation
        key_map.insert('-', (Key::KEY_MINUS, false));
        key_map.insert('=', (Key::KEY_EQUAL, false));
        key_map.insert('[', (Key::KEY_LEFTBRACE, false));
        key_map.insert(']', (Key::KEY_RIGHTBRACE, false));
        key_map.insert('\\', (Key::KEY_BACKSLASH, false));
        key_map.insert(';', (Key::KEY_SEMICOLON, false));
        key_map.insert('\'', (Key::KEY_APOSTROPHE, false));
        key_map.insert(',', (Key::KEY_COMMA, false));
        key_map.insert('.', (Key::KEY_DOT, false));
        key_map.insert('/', (Key::KEY_SLASH, false));
        key_map.insert('`', (Key::KEY_GRAVE, false));

        // Whitespace
        key_map.insert(' ', (Key::KEY_SPACE, false));
        key_map.insert('\t', (Key::KEY_TAB, false));
        key_map.insert('\n', (Key::KEY_ENTER, false));

        // Action keys, such as backspace, escape, ctrl, alt
        key_map.insert('\x08', (Key::KEY_BACKSPACE, false));
        key_map.insert('\x1b', (Key::KEY_ESC, false));

        key_map
    }

    fn key_down(&mut self, key: Key) -> Result<()> {
        self.device.emit(&[(InputEvent::new(EventType::KEY, key.code(), 1))])?;
        Ok(())
    }

    fn key_up(&mut self, key: Key) -> Result<()> {
        self.device.emit(&[(InputEvent::new(EventType::KEY, key.code(), 0))])?;
        Ok(())
    }

    fn string_to_keypresses(&mut self, input: &str) -> Result<(), evdev::Error> {
        for c in input.chars() {
            if let Some(&(key, shift)) = self.key_map.get(&c) {
                if shift {
                    // Press Shift
                    self.device.emit(&[InputEvent::new(
                            EventType::KEY,
                            Key::KEY_LEFTSHIFT.code(),
                            1,
                    )])?;
                }

                // Press key
                self.device.emit(&[InputEvent::new(EventType::KEY, key.code(), 1)])?;

                // Release key
                self.device.emit(&[InputEvent::new(EventType::KEY, key.code(), 0)])?;

                if shift {
                    // Release Shift
                    self.device.emit(&[InputEvent::new(
                            EventType::KEY,
                            Key::KEY_LEFTSHIFT.code(),
                            0,
                    )])?;
                }

                // Sync event
                self.device.emit(&[InputEvent::new(EventType::SYNCHRONIZATION, 0, 0)])?;
                thread::sleep(time::Duration::from_millis(10));
            }
        }

        Ok(())
    }

    fn key_cmd(&mut self, button: &str, shift: bool) -> Result<()> {
        self.key_down(Key::KEY_LEFTCTRL)?;
        if shift {
            self.key_down(Key::KEY_LEFTSHIFT)?;
        }
        self.string_to_keypresses(button)?;
        if shift {
            self.key_up(Key::KEY_LEFTSHIFT)?;
        }
        self.key_up(Key::KEY_LEFTCTRL)?;
        Ok(())
    }

    fn key_cmd_title(&mut self) -> Result<()> {
        self.key_cmd("1", false)?;
        Ok(())
    }

    fn key_cmd_subheading(&mut self) -> Result<()> {
        self.key_cmd("2", false)?;
        Ok(())
    }

    fn key_cmd_body(&mut self) -> Result<()> {
        self.key_cmd("3", false)?;
        Ok(())
    }

    fn key_cmd_bullet(&mut self) -> Result<()> {
        self.key_cmd("4", false)?;
        Ok(())
    }
}


