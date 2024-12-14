use anyhow::Result;
use ureq::Error;
use std::sync::{Arc, Mutex};

use serde_json::json;
use serde_json::Value as json;

use clap::{Parser, Subcommand};

use base64::prelude::*;

use std::thread::sleep;
use std::time::Duration;

use dotenv::dotenv;

use ghostwriter::{
    keyboard::Keyboard,
    screenshot::Screenshot,
    pen::Pen,
    touch::Touch,
    util::{svg_to_bitmap, write_bitmap_to_file},
    segmenter::analyze_image,
    engine::anthropic::Anthropic,
};

const REMARKABLE_WIDTH: u32 = 768;
const REMARKABLE_HEIGHT: u32 = 1024;

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

    /// Skip running draw_text or draw_svg
    #[arg(long)]
    no_draw: bool,

    /// Disable keyboard progress
    #[arg(long)]
    no_draw_progress: bool,

    /// Input PNG file for testing
    #[arg(long)]
    input_png: Option<String>,

    /// Output file for testing
    #[arg(long)]
    output_file: Option<String>,

    /// Output file for model parameters
    #[arg(long)]
    model_output_file: Option<String>,

    /// Save screenshot filename
    #[arg(long)]
    save_screenshot: Option<String>,

    /// Save bitmap filename
    #[arg(long)]
    save_bitmap: Option<String>,

    /// Disable looping
    #[arg(long)]
    no_loop: bool,

    /// Apply segmentation
    #[arg(long)]
    apply_segmentation: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    TextAssist,
    ClaudeAssist,
}

fn main() -> Result<()> {
    dotenv().ok();
    let args = Args::parse();

    match &args.command {
        Some(Command::ClaudeAssist) => claude_assist(&args),
        Some(Command::TextAssist) | None => ghostwriter(&args),
    }
}

fn ghostwriter(args: &Args) -> Result<()> {
    let mut keyboard = Keyboard::new(args.no_draw, args.no_draw_progress);
    let mut pen = Pen::new(args.no_draw);
    let mut touch = Touch::new(args.no_draw);

    // Default to regular text size
    keyboard.key_cmd_body()?;

    loop {
        if let Some(input_png) = &args.input_png {
            println!("Using input PNG file: {}", input_png);
        } else {
            println!("Waiting for trigger (hand-touch in the upper-right corner)...");
            touch.wait_for_trigger()?;
        }

        keyboard.progress()?;

        let base64_image = if let Some(input_png) = &args.input_png {
            BASE64_STANDARD.encode(std::fs::read(input_png)?)
        } else {
            let screenshot = Screenshot::new()?;
            if let Some(save_screenshot) = &args.save_screenshot {
                screenshot.save_image(save_screenshot)?;
            }
            screenshot.base64()?
        };
        keyboard.progress()?;

        if args.no_submit {
            println!("Image not submitted to OpenAI due to --no-submit flag");
            keyboard.progress_end()?;
            return Ok(());
        }

        let api_key = std::env::var("OPENAI_API_KEY")?;
        let tools = json!([
        {
            "type": "function",
            "function": {
                "name": "draw_text",
                "description": "Draw text to the screen using simulated keyboard input. The input_description and output_description are used to build a plan for the actual output.",
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
                "name": "draw_svg",
                "description": "Draw an SVG to the screen using simulated pen input. The input_description and output_description are used to build a plan for the actual output.",
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
                            "description": "SVG data to be rendered. This is drawn on top of the input image, and should be the same size as the input image (768x1024 px). The display can only show black and white. Try to place the output in an integrated position. Use the `Noto Sans` font-family when you are showing text. Do not use a style tag tag. Do not use any fill colors or gradients or transparency or shadows. Do include the xmlns in the main svg tag."
                        }
                    },
                    "required": ["input_description", "output_description", "svg"]
                }
            }
        }
        ]);

        let body = json!({
            "model": args.model,
            "messages": [{
                "role": "user",
                "content": [
                {
                    "type": "text",
                    "text": "You are a helpful assistant. You live inside of a remarkable2 notepad, which has a 768x1024 px sized screen which can only display grayscale. Your input is the current content of the screen, which may contain content written by the user or previously written by you (the assistant). Look at this content, interpret it, and respond to the content. The content will contain handwritten notes, diagrams, and maybe typewritten text. Respond by calling a tool. Call draw_text to output text which will be sent using simulated keyboard input. Call draw_svg to respond with an SVG drawing which will be drawn on top of the existing content. Try to place the output on the screen at coordinates that make sense. If you need to place text at a very specific location, you should output an SVG instead of keyboard text."
                },

                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/png;base64,{}", base64_image)
                    }
                }
                ]
            }],
            "tools": tools,
            "tool_choice": "required",
            "parallel_tool_calls": false
        });

        keyboard.progress()?;
        let response = ureq::post("https://api.openai.com/v1/chat/completions")
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(&body)?;
        keyboard.progress()?;

        let json: serde_json::Value = response.into_json()?;
        println!("Response: {}", json);
        let tool_calls = &json["choices"][0]["message"]["tool_calls"];

        if let Some(tool_call) = tool_calls.get(0) {
            keyboard.progress()?;
            let function_name = tool_call["function"]["name"].as_str().unwrap();
            let arguments = tool_call["function"]["arguments"].as_str().unwrap();
            let json_output = serde_json::from_str::<serde_json::Value>(arguments)?;
            keyboard.progress()?;

            match function_name {
                "draw_text" => {
                    let text = json_output["text"].as_str().unwrap();
                    if let Some(output_file) = &args.output_file {
                        std::fs::write(output_file, text)?;
                    }
                    if !args.no_draw {
                        draw_text(text, &mut keyboard)?;
                    }
                    if let Some(model_output_file) = &args.model_output_file {
                        let params = json!({
                            "function": function_name,
                            "arguments": json_output
                        });
                        std::fs::write(model_output_file, params.to_string())?;
                    }
                }
                "draw_svg" => {
                    let svg_data = json_output["svg"].as_str().unwrap();
                    if let Some(output_file) = &args.output_file {
                        std::fs::write(output_file, svg_data)?;
                    }
                    draw_svg(
                        svg_data,
                        &mut keyboard,
                        &mut pen,
                        args.save_bitmap.as_ref(),
                        args.no_draw,
                    )?;
                    if let Some(model_output_file) = &args.model_output_file {
                        let params = json!({
                            "function": function_name,
                            "arguments": json_output
                        });
                        std::fs::write(model_output_file, params.to_string())?;
                    }
                }
                _ => {
                    keyboard.progress_end()?;
                    return Err(anyhow::anyhow!("Unknown function called"));
                }
            }
        } else {
            keyboard.progress_end()?;
            return Err(anyhow::anyhow!("No tool call found in response"));
        }

        if args.no_loop {
            break Ok(());
        }
    }
}

fn draw_text(text: &str, keyboard: &mut Keyboard) -> Result<()> {
    keyboard.progress()?;
    keyboard.progress_end()?;
    keyboard.key_cmd_body()?;
    keyboard.string_to_keypresses(text)?;
    keyboard.string_to_keypresses("\n\n")?;
    Ok(())
}

fn draw_svg(
    svg_data: &str,
    keyboard: &mut Keyboard,
    pen: &mut Pen,
    save_bitmap: Option<&String>,
    no_draw: bool,
) -> Result<()> {
    keyboard.progress()?;
    let bitmap = svg_to_bitmap(svg_data, REMARKABLE_WIDTH, REMARKABLE_HEIGHT)?;
    if let Some(save_bitmap) = save_bitmap {
        write_bitmap_to_file(&bitmap, save_bitmap)?;
    }
    if !no_draw {
        pen.draw_bitmap(&bitmap)?;
    }
    keyboard.progress_end()?;
    Ok(())
}

fn claude_assist(args: &Args) -> Result<()> {
    let keyboard = Arc::new(Mutex::new(Keyboard::new(args.no_draw, args.no_draw_progress)));
    let pen = Arc::new(Mutex::new(Pen::new(args.no_draw)));
    let mut touch = Touch::new(args.no_draw);

    let api_key = std::env::var("ANTHROPIC_API_KEY")?;
    let model = "claude-3-5-sonnet-latest";

    let mut engine = Anthropic::new(model.to_string());

    let output_file = args.output_file.clone();
    let no_draw = args.no_draw;
    let keyboard_clone = Arc::clone(&keyboard);
    engine.register_tool(
        "draw_text",
        serde_json::from_str::<serde_json::Value>(include_str!("../prompts/tool_draw_text.json"))?,
        Box::new(move |arguments: json| {
            let text = arguments["text"].as_str().unwrap();
            if let Some(output_file) = &output_file {
                std::fs::write(output_file, text).unwrap();
            }
            if !no_draw {
                let mut keyboard = keyboard_clone.lock().unwrap();
                draw_text(text, &mut keyboard).unwrap();
            }
        }),
    );

    let output_file = args.output_file.clone();
    let save_bitmap = args.save_bitmap.clone();
    let no_draw = args.no_draw;
    let keyboard_clone = Arc::clone(&keyboard);
    let pen_clone = Arc::clone(&pen);
    engine.register_tool(
        "draw_svg",
        serde_json::from_str::<serde_json::Value>(include_str!("../prompts/tool_draw_svg.json"))?,
        Box::new(move |arguments: json| {
            let svg_data = arguments["svg"].as_str().unwrap();
            if let Some(output_file) = &output_file {
                std::fs::write(output_file, svg_data).unwrap();
            }
            let mut keyboard = keyboard_clone.lock().unwrap();
            let mut pen = pen_clone.lock().unwrap();
            draw_svg(
                svg_data,
                &mut keyboard,
                &mut pen,
                save_bitmap.as_ref(),
                no_draw,
            ).unwrap();
        }),
    );

    loop {
        if let Some(input_png) = &args.input_png {
            println!("Using input PNG file: {}", input_png);
        } else {
            println!("Waiting for trigger (hand-touch in the upper-right corner)...");
            touch.wait_for_trigger()?;
        }

        keyboard.lock().unwrap().progress()?;

        let base64_image = if let Some(input_png) = &args.input_png {
            BASE64_STANDARD.encode(std::fs::read(input_png)?)
        } else {
            let screenshot = Screenshot::new()?;
            if let Some(save_screenshot) = &args.save_screenshot {
                screenshot.save_image(save_screenshot)?;
            }
            screenshot.base64()?
        };
        keyboard.lock().unwrap().progress()?;

        if args.no_submit {
            println!("Image not submitted to OpenAI due to --no-submit flag");
            keyboard.lock().unwrap().progress_end()?;
            return Ok(());
        }

        let segmentation_description = if args.apply_segmentation {
            let input_filename = args
                .input_png
                .clone()
                .unwrap_or(args.save_screenshot.clone().unwrap());
            match analyze_image(input_filename.as_str()) {
                Ok(description) => description,
                Err(e) => format!("Error analyzing image: {}", e),
            }
        } else {
            String::new()
        };
        println!("Segmentation description: {}", segmentation_description);

        engine.clear_content();
        engine.add_content(
            json!({
                "type": "text",
                "text": "You are a helpful assistant. You live inside of a remarkable2 notepad, which has a 768x1024 px sized screen which can only display grayscale. Your input is the current content of the screen, which may contain content written by the user or previously written by you (the assistant). Look at this content, interpret it, and respond to the content. The content will contain handwritten notes, diagrams, and maybe typewritten text. Respond by calling a tool. Call draw_text to output text which will be sent using simulated keyboard input. Call draw_svg to respond with an SVG drawing which will be drawn on top of the existing content. Try to place the output on the screen at coordinates that make sense. If you need to place text at a very specific location, you should output an SVG instead of keyboard text."
            })
        );

        if args.apply_segmentation {
            engine.add_content(
                json!({
                    "type": "text",
                    "text": format!("Here are interesting regions based on an automatic segmentation algorithm. Use them to help identify the exact location of interesting features.\n\n{}", segmentation_description)
                })
            );
        }

        engine.add_content(
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": base64_image
                }
            })
        );

        engine.execute()?;

        if args.no_loop {
            break Ok(());
        }
    }
}
