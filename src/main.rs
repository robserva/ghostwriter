use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::io::Write;
use image::{GrayImage};

use std::io::{BufWriter};
use byteorder::{BigEndian, WriteBytesExt};
use clap::Parser;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;
const REMARKABLE_BYTES_PER_PIXEL: usize = 2;

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
                        "text": "What's in this image?"
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
        },
        Err(ureq::Error::Status(code, response)) => {
            println!("HTTP Error: {} {}", code, response.status_text());
            if let Ok(json) = response.into_json::<serde_json::Value>() {
                println!("Error details: {}", json);
            } else {
                println!("Failed to parse error response as JSON");
            }
            return Err(anyhow::anyhow!("API request failed"));
        },
        Err(e) => return Err(anyhow::anyhow!("Request failed: {}", e)),
    }
    Ok(())
}use std::process::Command;

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
    let output = Command::new("pidof")
        .arg("xochitl")
        .output()?;
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
        .arg(format!("grep -C1 '/dev/fb0' /proc/{}/maps | tail -n1 | sed 's/-.*$//'", pid))
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



fn encode_png(raw_data: &[u8]) -> Result<Vec<u8>> {
    // Write raw data as 16-bit PGM file for inspection
    let pgm_filename = "tmp/screenshot_raw.pgm";
    let pgm_file = File::create(pgm_filename)?;
    let mut writer = BufWriter::new(pgm_file);

    writeln!(writer, "P5")?;
    writeln!(writer, "{} {}", REMARKABLE_WIDTH, REMARKABLE_HEIGHT)?;
    writeln!(writer, "65535")?;

    for chunk in raw_data.chunks_exact(2) {
        let value = u16::from_be_bytes([chunk[0], chunk[1]]);
        writer.write_u16::<BigEndian>(value)?;
    }
    writer.flush()?;
    println!("16-bit raw grayscale data saved as PGM to {}", pgm_filename);

    // Proceed with PNG encoding (you may need to adjust this part as well)
    let img = GrayImage::from_raw(REMARKABLE_WIDTH, REMARKABLE_HEIGHT, raw_data.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Failed to create image from raw data"))?;

    let mut png_data = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new(&mut png_data);
    encoder.encode(
        img.as_raw(),
        REMARKABLE_WIDTH,
        REMARKABLE_HEIGHT,
        image::ColorType::L16,
    )?;

    Ok(png_data)
}


