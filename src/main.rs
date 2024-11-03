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
        Some(Command::TextAssist) | None => ghostwriter(&args),
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

fn ghostwriter(args: &Args) -> Result<()> {
    let mut keyboard = Keyboard::new();
    let mut pen = Pen::new();
    let mut touch = Touch::new();

    loop {
        println!("Waiting for trigger (hand-touch in the upper-right corner)...");
        touch.wait_for_trigger()?;

        // TODO: Show progress indicator using the keyboard in all cases? Some other cool doodle?

        let screenshot = Screenshot::new()?;
        screenshot.save_image("tmp/screenshot.png")?;
        let base64_image = screenshot.base64()?;

        if args.no_submit {
            println!("Image not submitted to OpenAI due to --no-submit flag");
            return Ok(());
        }

        let api_key = std::env::var("OPENAI_API_KEY")?;
        let tools = json!([
            {
                "type": "function",
                "function": {
                    "name": "process_text",
                    "description": "Process text from the image and return structured text output",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "input_description": {
                                "type": "string",
                                "description": "Description of what was detected in the input image"
                            },
                            "output_description": {
                                "type": "string",
                                "description": "Description of what will be output"
                            },
                            "text": {
                                "type": "string",
                                "description": "Text to be written"
                            }
                        },
                        "required": ["input_description", "output_description", "text"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "process_drawing",
                    "description": "Process the drawing and return structured SVG output",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "input_description": {
                                "type": "string",
                                "description": "Description of what was detected in the input image"
                            },
                            "output_description": {
                                "type": "string",
                                "description": "Description of what will be drawn"
                            },
                            "svg": {
                                "type": "string",
                                "description": "SVG data to be rendered"
                            }
                        },
                        "required": ["input_description", "output_description", "svg"]
                    }
                }
            }
        ]);

        let mut body = json!({
            "model": args.model,
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/png;base64,{}", base64_image)
                    }
                }]
            }],
            "tools": tools,
            "tool_choice": "auto"
        });

        // Process response and handle either text or drawing output based on the tool called
        match handle_api_response(&api_key, body, &mut keyboard, &mut pen)? {
            OutputType::Text => println!("Processed text output"),
            OutputType::Drawing => println!("Processed drawing output"),
        }
    }
}

enum OutputType {
    Text,
    Drawing,
}

fn handle_api_response(
    api_key: &str, 
    body: serde_json::Value,
    keyboard: &mut Keyboard,
    pen: &mut Pen,
) -> Result<OutputType> {
    let response = ureq::post("https://api.openai.com/v1/chat/completions")
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .send_json(&body)?;

    let json: serde_json::Value = response.into_json()?;
    let tool_calls = &json["choices"][0]["message"]["tool_calls"];
    
    if let Some(tool_call) = tool_calls.get(0) {
        let function_name = tool_call["function"]["name"].as_str().unwrap();
        let arguments = tool_call["function"]["arguments"].as_str().unwrap();
        let json_output = serde_json::from_str::<serde_json::Value>(arguments)?;

        match function_name {
            "process_text" => {
                let text = json_output["text"].as_str().unwrap();
                keyboard.key_cmd_body()?;
                keyboard.string_to_keypresses(text)?;
                keyboard.string_to_keypresses("\n\n")?;
                Ok(OutputType::Text)
            },
            "process_drawing" => {
                let svg_data = json_output["svg"].as_str().unwrap();
                let bitmap = svg_to_bitmap(svg_data, REMARKABLE_WIDTH, REMARKABLE_HEIGHT)?;
                draw_bitmap(pen, &bitmap)?;
                Ok(OutputType::Drawing)
            },
            _ => Err(anyhow::anyhow!("Unknown function called"))
        }
    } else {
        Err(anyhow::anyhow!("No tool call found in response"))
    }
}

fn draw_bitmap(pen: &mut Pen, bitmap: &Vec<Vec<bool>>) -> Result<()> {
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
            } else if is_pen_down {
                pen.pen_up()?;
                is_pen_down = false;
                sleep(Duration::from_millis(1));
            }
        }
        pen.pen_up()?;
        is_pen_down = false;
        sleep(Duration::from_millis(5));
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



