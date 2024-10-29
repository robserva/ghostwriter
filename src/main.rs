use anyhow::Result;

use image::GrayImage;
use serde_json::json;

use clap::{Parser, Subcommand};

use resvg::render;
use resvg::tiny_skia::Pixmap;
use resvg::usvg;
use resvg::usvg::{fontdb, Options, Tree};
use std::sync::Arc;

use std::thread::sleep;
use std::time::Duration;

mod keyboard;
use crate::keyboard::Keyboard;

mod screenshot;
use crate::screenshot::Screenshot;

mod pen;
use crate::pen::Pen;

mod touch;
use crate::touch::Touch;

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
    let mut touch = Touch::new();

    loop {
        println!("Waiting for trigger (hand-touch in the upper-right corner)...");
        touch.wait_for_trigger()?;

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
    let mut pen = Pen::new();
    let mut touch = Touch::new();

    loop {
        println!("Waiting for trigger (hand-touch in the upper-right corner)...");
        touch.wait_for_trigger()?;

        let screenshot = Screenshot::new()?;

        pen.draw_line_screen((1340, 5), (1390, 75))?;

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
        pen.draw_line_screen((1340, 75), (1390, 5))?;

        let response = ureq::post("https://api.openai.com/v1/chat/completions")
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(&body);

        match response {
            Ok(response) => {
                let json: serde_json::Value = response.into_json()?;
                println!("API Response: {}", json);
                pen.draw_line_screen( (1365, 5), (1365, 75))?;

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
                                pen.goto_xy_screen((x as i32, y as i32))?;
                                pen.pen_down()?;
                                is_pen_down = true;
                                sleep(Duration::from_millis(1));
                            }
                            pen.goto_xy_screen((x as i32, y as i32))?;
                            pen.goto_xy_screen((x as i32 + 1, y as i32))?;
                        } else {
                            if is_pen_down {
                                pen.pen_up()?;
                                is_pen_down = false;
                                sleep(Duration::from_millis(1));
                            }
                        }
                    }

                    // At the end of the row, pick up the pen no matter what
                    pen.pen_up()?;
                    is_pen_down = false;
                    sleep(Duration::from_millis(5));
                }

                pen.draw_line_screen( (1330, 40), (1390, 40))?;
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



