use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use image::GrayImage;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::io::{Read, Seek};


use clap::Parser;


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

    let screenshot_data = take_screenshot()?;

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
    let body = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "You are a helpful assistant. You live inside of a remarkable2 notepad. Your input is the current state of the screen. Look at this screenshot and use it as your input. Respond in text; later we will figure out how to turn your responses into handwriting on the screen."
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
        "max_tokens": 300
    });

    let response = ureq::post("https://api.openai.com/v1/chat/completions")
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .send_json(&body);

    match response {
        Ok(response) => {
            let json: serde_json::Value = response.into_json()?;
            println!("API Response: {}", json);
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
    Ok(())
}
use std::process::Command;

const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;
const BYTES_PER_PIXEL: usize = 2;
const WINDOW_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;

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
    let raw_u8: Vec<u8> = raw_data.chunks_exact(2)
        .map(|chunk| u8::from_le_bytes([chunk[1]]))
        .collect();

    let mut processed = vec![0u8; (REMARKABLE_WIDTH * REMARKABLE_HEIGHT) as usize];

    for y in 0..REMARKABLE_HEIGHT {
        for x in 0..REMARKABLE_WIDTH {
            // let src_idx = y * REMARKABLE_WIDTH + x;
            // let dst_idx = x * REMARKABLE_HEIGHT + (REMARKABLE_HEIGHT - 1 - y);
            let src_idx = (REMARKABLE_HEIGHT - 1 - y) + (REMARKABLE_WIDTH - 1 - x) * REMARKABLE_HEIGHT;
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
        image::ColorType::L8
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
