use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use image::GrayImage;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::io::{Read, Seek};
use std::{thread, time};

use clap::Parser;

use resvg::render;
use resvg::tiny_skia::Pixmap;
use resvg::usvg;
use resvg::usvg::{fontdb, Options, Tree};
use std::sync::Arc;

use std::process::Command;

use evdev::{Device, EventType, InputEvent, InputEventKind};

use std::os::unix::io::AsRawFd;


const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;
const BYTES_PER_PIXEL: usize = 2;
const WINDOW_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;
const INPUT_WIDTH: usize = 15725;
const INPUT_HEIGHT: usize = 20966;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    no_submit: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Open the device for drawing
    let mut device = Device::open("/dev/input/event1")?;

    loop {

    wait_for_trigger()?;
                
                

    let screenshot_data = take_screenshot()?;

    draw_line(&mut device, screen_to_input((1340, 5)), screen_to_input((1390, 75)))?;

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

    // Example: Draw a simple line
    // let points = vec![(100, 100), (200, 200), (300, 300)];
    // draw_on_screen()?;

    if args.no_submit {
        println!("Image not submitted to OpenAI due to --no-submit flag");
        return Ok(());
    }

    let api_key = std::env::var("OPENAI_API_KEY")?;
    let body = json!({
        "model": "gpt-4o-mini",
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "strict": true,
                "name": "svg_response",
                "schema": {
                    "type": "object",
                    "properties": {
                        "input_description": {
                            "type": "string",
                            "description": "Description of input, including interpretation of what is being asked and interesting features"
                        },
                        "input_features": {
                            "type": "array",
                            "description": "List of features in the input, including their description and coordinates",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "description": {
                                        "type": "string",
                                        "description": "Description of feature"
                                    },
                                    "top_left_x": {
                                        "type": "number",
                                        "description": "Top-left corner X coordinate of feature"
                                    },
                                    "top_left_y": {
                                        "type": "number",
                                        "description": "Top-left corner Y coordinate of feature"
                                    },
                                    "bottom_right_x": {
                                        "type": "number",
                                        "description": "Bottom-right corner X coordinate of feature"
                                    },
                                    "bottom_right_y": {
                                        "type": "number",
                                        "description": "Bottom-right corner Y coordinate of feature"
                                    },
                                },
                                "required": ["description", "top_left_x", "top_left_y", "bottom_right_x", "bottom_right_y"],
                                "additionalProperties": false
                            },          
                        },
                        "output_description": {
                            "type": "string",
                            "description": "Description of response, both in general and specifics about how the response can be represented in an SVG overlayed on the screen. Include specifics such as position in coordinates of response objects"
                        },
                        "svg": {
                            "type": "string",
                            "description": "An SVG in correct SVG format which will be drawn on top of the existing screen elements"
                        }
                    },
                    "required": [
                        "input_description",
                        "input_features",
                        "output_description",
                        "svg"
                    ],
                    "additionalProperties": false
                }
            }
        },
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "You are a helpful assistant. You live inside of a remarkable2 notepad, which has a 1404x1872 sized screen which can only display black and white. Your input is the current content of the screen. Look at this content, interpret it, and respond to the content. The content will contain both handwritten notes and diagrams. Respond in the form of a JSON document which will explain the input, the output, and provide an actual svg, which we will draw onto the same screen, on top of the existing content. Try to place the output in an integrated position. Use the `Noto Sans` font-family when you are showing text."
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", base64_image)
                        }
                    }
                ]
            }
        ],
        "max_tokens": 3000
    });

    println!("Sending request to OpenAI API...");
    draw_line(&mut device, screen_to_input((1340, 75)), screen_to_input((1390, 5)))?;
    
    let response = ureq::post("https://api.openai.com/v1/chat/completions")
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .send_json(&body);

    match response {
        Ok(response) => {
            let json: serde_json::Value = response.into_json()?;
            println!("API Response: {}", json);
            draw_line(&mut device, screen_to_input((1365, 5)), screen_to_input((1365, 75)))?;

            let raw_output = json["choices"][0]["message"]["content"].as_str().unwrap();
            let json_output = serde_json::from_str::<serde_json::Value>(raw_output)?;
            let input_description = json_output["input_description"].as_str().unwrap();
            let output_description = json_output["output_description"].as_str().unwrap();
            let svg_data = json_output["svg"].as_str().unwrap();
            let bitmap =
                svg_to_bitmap(svg_data, REMARKABLE_WIDTH as u32, REMARKABLE_HEIGHT as u32)?;
            write_bitmap_to_file(&bitmap, "tmp/debug_bitmap.png")?;



            // Iterate through the bitmap and draw dots where needed
            for (y, row) in bitmap.iter().enumerate() {
                for (x, &pixel) in row.iter().enumerate() {
                    if pixel {
                        draw_dot(&mut device, screen_to_input((x as i32, y as i32)))?;
                    }
                }
            }

            println!("Input Description: {}", input_description);
            println!("Output Description: {}", output_description);
            println!("SVG Data: {}", svg_data);
            
            draw_line(&mut device, screen_to_input((1330, 40)), screen_to_input((1390, 40)))?;
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
    let output = Command::new("pidof").arg("xochitl").output()?;
    let pids = String::from_utf8(output.stdout)?;
    for pid in pids.split_whitespace() {
        let has_fb = Command::new("grep")
            .args(&["-C1", "/dev/fb0", &format!("/proc/{}/maps", pid)])
            .output()?;
        if !has_fb.stdout.is_empty() {
            return Ok(pid.to_string());
        }
    }
    anyhow::bail!("No xochitl process with /dev/fb0 found")
}

fn find_framebuffer_address(pid: &str) -> Result<u64> {
    let output = Command::new("sh")
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
            // let src_idx = y * REMARKABLE_WIDTH + x;
            // let dst_idx = x * REMARKABLE_HEIGHT + (REMARKABLE_HEIGHT - 1 - y);
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
    println!("Drawing from ({}, {}) to ({}, {})", x1, y1, x2, y2);

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
    println!("Drawing from ({}, {}) to ({}, {}) in {} steps", x1, y1, x2, y2, steps);

    draw_pen_up(device)?;
    draw_goto_xy(device, (x1, y1))?;
    draw_pen_down(device)?;


    for i in 0..steps {
        let x = x1 + dx * i;
        let y = y1 + dy * i;
        draw_goto_xy(device, (x, y))?;
        println!("Drawing to point at ({}, {})", x, y);

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
        draw_goto_xy(device, (x+n, y+n))?;
    }

    draw_pen_up(device)?;

    // sleep for 5ms
    thread::sleep(time::Duration::from_millis(1));

    Ok(())
}

fn draw_pen_down(device: &mut Device) -> Result<()> {
    device.send_events(&[
        InputEvent::new(EventType::KEY, 320, 1),     // BTN_TOOL_PEN
        InputEvent::new(EventType::KEY, 330, 1),     // BTN_TOUCH
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

fn draw_on_screen() -> Result<()> {
    let mut device = Device::open("/dev/input/event1")?; // Pen input device

    for x in 0..100 {
        draw_dot(&mut device, screen_to_input((1000 + x, 1000)))?;
    }

    Ok(())
}

fn svg_to_bitmap(svg_data: &str, width: u32, height: u32) -> Result<Vec<Vec<bool>>> {
    let mut opt = Options::default();
    let mut fontdb = fontdb::Database::new();
    fontdb.load_fonts_dir("/usr/share/fonts/ttf/noto");
    opt.fontdb = Arc::new(fontdb);

    let tree = Tree::from_str(svg_data, &opt)?;
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

/*
Trace from evtest

Input driver version is 1.0.1
Input device ID: bus 0x0 vendor 0x0 product 0x0 version 0x0
Input device name: "pt_mt"
Supported events:
  Event type 0 (EV_SYN)
  Event type 1 (EV_KEY)
    Event code 59 (KEY_F1)
    Event code 60 (KEY_F2)
    Event code 61 (KEY_F3)
    Event code 62 (KEY_F4)
    Event code 63 (KEY_F5)
    Event code 64 (KEY_F6)
    Event code 65 (KEY_F7)
    Event code 66 (KEY_F8)
  Event type 2 (EV_REL)
  Event type 3 (EV_ABS)
    Event code 25 (ABS_DISTANCE)
      Value      0
      Min        0
      Max      255
    Event code 47 (ABS_MT_SLOT)
      Value      0
      Min        0
      Max       31
    Event code 48 (ABS_MT_TOUCH_MAJOR)
      Value      0
      Min        0
      Max      255
    Event code 49 (ABS_MT_TOUCH_MINOR)
      Value      0
      Min        0
      Max      255
    Event code 52 (ABS_MT_ORIENTATION)
      Value      0
      Min     -127
      Max      127
    Event code 53 (ABS_MT_POSITION_X)
      Value      0
      Min        0
      Max     1403
    Event code 54 (ABS_MT_POSITION_Y)
      Value      0
      Min        0
      Max     1871
    Event code 55 (ABS_MT_TOOL_TYPE)
      Value      0
      Min        0
      Max        1
    Event code 57 (ABS_MT_TRACKING_ID)
      Value      0
      Min        0
      Max    65535
    Event code 58 (ABS_MT_PRESSURE)
      Value      0
      Min        0
      Max      255
Properties:
  Property type 1 (INPUT_PROP_DIRECT)



Event: time 1728350196.278103, type 3 (EV_ABS), code 57 (ABS_MT_TRACKING_ID), value 2879
Event: time 1728350196.278103, type 3 (EV_ABS), code 53 (ABS_MT_POSITION_X), value 1340
Event: time 1728350196.278103, type 3 (EV_ABS), code 54 (ABS_MT_POSITION_Y), value 50
Event: time 1728350196.278103, type 3 (EV_ABS), code 58 (ABS_MT_PRESSURE), value 164
Event: time 1728350196.278103, type 3 (EV_ABS), code 48 (ABS_MT_TOUCH_MAJOR), value 17
Event: time 1728350196.278103, type 3 (EV_ABS), code 49 (ABS_MT_TOUCH_MINOR), value 26
Event: time 1728350196.278103, type 3 (EV_ABS), code 52 (ABS_MT_ORIENTATION), value 5
Event: time 1728350196.278103, -------------- SYN_REPORT ------------
Event: time 1728350196.340895, type 3 (EV_ABS), code 53 (ABS_MT_POSITION_X), value 1341
Event: time 1728350196.340895, type 3 (EV_ABS), code 54 (ABS_MT_POSITION_Y), value 52
Event: time 1728350196.340895, type 3 (EV_ABS), code 58 (ABS_MT_PRESSURE), value 170
Event: time 1728350196.340895, type 3 (EV_ABS), code 48 (ABS_MT_TOUCH_MAJOR), value 26
Event: time 1728350196.340895, type 3 (EV_ABS), code 52 (ABS_MT_ORIENTATION), value 6
Event: time 1728350196.340895, -------------- SYN_REPORT ------------
Event: time 1728350196.352804, type 3 (EV_ABS), code 54 (ABS_MT_POSITION_Y), value 53
Event: time 1728350196.352804, type 3 (EV_ABS), code 58 (ABS_MT_PRESSURE), value 167
Event: time 1728350196.352804, -------------- SYN_REPORT ------------
Event: time 1728350196.364653, type 3 (EV_ABS), code 53 (ABS_MT_POSITION_X), value 1342
Event: time 1728350196.364653, type 3 (EV_ABS), code 54 (ABS_MT_POSITION_Y), value 55
Event: time 1728350196.364653, type 3 (EV_ABS), code 58 (ABS_MT_PRESSURE), value 155
Event: time 1728350196.364653, type 3 (EV_ABS), code 49 (ABS_MT_TOUCH_MINOR), value 17
Event: time 1728350196.364653, type 3 (EV_ABS), code 52 (ABS_MT_ORIENTATION), value 4
Event: time 1728350196.364653, -------------- SYN_REPORT ------------
Event: time 1728350196.376561, type 3 (EV_ABS), code 53 (ABS_MT_POSITION_X), value 1343
Event: time 1728350196.376561, type 3 (EV_ABS), code 54 (ABS_MT_POSITION_Y), value 59
Event: time 1728350196.376561, type 3 (EV_ABS), code 58 (ABS_MT_PRESSURE), value 110
Event: time 1728350196.376561, -------------- SYN_REPORT ------------
Event: time 1728350196.411901, type 3 (EV_ABS), code 57 (ABS_MT_TRACKING_ID), value -1
Event: time 1728350196.411901, -------------- SYN_REPORT ------------
*/

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
