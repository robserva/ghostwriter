use anyhow::Result;
use ureq::Error;

use serde_json::json;

use clap::{Parser, Subcommand};


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

mod util;
use crate::util::{svg_to_bitmap};

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
    ClaudeAssist,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match &args.command {
        Some(Command::KeyboardTest) => keyboard_test(),
        Some(Command::ClaudeAssist) => claude_assist(&args),
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

    // Default to regular text size
    keyboard.key_cmd_body()?;

    loop {
        println!("Waiting for trigger (hand-touch in the upper-right corner)...");
        touch.wait_for_trigger()?;

        keyboard.progress()?;

        // TODO: Show progress indicator using the keyboard in all cases? Some other cool doodle?

        let screenshot = Screenshot::new()?;
        screenshot.save_image("tmp/screenshot.png")?;
        let base64_image = screenshot.base64()?;
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
                            "description": "SVG data to be rendered. This is drawn on top of the input image, and should be the same size as the input image (1404x1872 px). The display can only show black and white. Try to place the output in an integrated position. Use the `Noto Sans` font-family when you are showing text. Do not use a style tag tag. Do not use any fill colors or gradients or transparency or shadows. Do include the xmlns in the main svg tag."
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
                    "text": "You are a helpful assistant. You live inside of a remarkable2 notepad, which has a 1404x1872 sized screen which can only display grayscale. Your input is the current content of the screen, which may contain content written by the user or previously written by you (the assistant). Look at this content, interpret it, and respond to the content. The content will contain handwritten notes, diagrams, and maybe typewritten text. Respond by calling a tool. Call draw_text to output text which will be sent using simulated keyboard input. Call draw_svg to respond with an SVG drawing which will be drawn on top of the existing content. Try to place the output on the screen at coordinates that make sense. If you need to place text at a very specific location, you should output an SVG instead of keyboard text."
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
                    draw_text(text, &mut keyboard)?;

                }
                "draw_svg" => {
                    let svg_data = json_output["svg"].as_str().unwrap();
                    draw_svg(svg_data, &mut keyboard, &mut pen)?;
                }
                _ => {
                    keyboard.progress_end()?;
                    return Err(anyhow::anyhow!("Unknown function called"))
                }
            }
        } else {
            keyboard.progress_end()?;
            return Err(anyhow::anyhow!("No tool call found in response"))
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

fn draw_svg(svg_data: &str, keyboard: &mut Keyboard, pen: &mut Pen) -> Result<()> {
    keyboard.progress()?;
    let bitmap = svg_to_bitmap(svg_data, REMARKABLE_WIDTH, REMARKABLE_HEIGHT)?;
    keyboard.progress()?;
    pen.draw_bitmap(&bitmap)?;
    keyboard.progress_end()?;
    Ok(())
}


fn claude_assist(args: &Args) -> Result<()> {
    let mut keyboard = Keyboard::new();
    let mut pen = Pen::new();
    let mut touch = Touch::new();

    // Default to regular text size
    keyboard.key_cmd_body()?;

    loop {
        println!("Waiting for trigger (hand-touch in the upper-right corner)...");
        touch.wait_for_trigger()?;

        keyboard.progress()?;

        // TODO: Show progress indicator using the keyboard in all cases? Some other cool doodle?

        let screenshot = Screenshot::new()?;
        screenshot.save_image("tmp/screenshot.png")?;
        let base64_image = screenshot.base64()?;
        keyboard.progress()?;

        if args.no_submit {
            println!("Image not submitted to OpenAI due to --no-submit flag");
            keyboard.progress_end()?;
            return Ok(());
        }

        let api_key = std::env::var("ANTHROPIC_API_KEY")?;
        let tools = json!([
        {
            "name": "draw_text",
            "description": "Draw text to the screen using simulated keyboard input. The input_description and output_description are used to build a plan for the actual output.",
            "input_schema": {
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
        },
        {
            "name": "draw_svg",
            "description": "Draw an SVG to the screen using simulated pen input. The input_description and output_description are used to build a plan for the actual output.",
            "input_schema": {
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
                        "description": "SVG data to be rendered. This is drawn on top of the input image, and should be the same size as the input image (1404x1872 px). The display can only show black and white. Try to place the output in an integrated position. Use the `Noto Sans` font-family when you are showing text. Do not use a style tag tag. Do not use any fill colors or gradients or transparency or shadows. Do include the xmlns in the main svg tag."
                    }
                },
                "required": ["input_description", "output_description", "svg"]
            }
        }
        ]);

        let body = json!({
            // "model": "args.model,
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 5000,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "You are a helpful assistant. You live inside of a remarkable2 notepad, which has a 1404x1872 sized screen which can only display grayscale. Your input is the current content of the screen, which may contain content written by the user or previously written by you (the assistant). Look at this content, interpret it, and respond to the content. The content will contain handwritten notes, diagrams, and maybe typewritten text. Respond by calling a tool. Call draw_text to output text which will be sent using simulated keyboard input. Call draw_svg to respond with an SVG drawing which will be drawn on top of the existing content. Try to place the output on the screen at coordinates that make sense. If you need to place text at a very specific location, you should output an SVG instead of keyboard text."
                    },
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": base64_image
                        }
                    }
                ]
            }],
            "tools": tools,
            "tool_choice": {
                "type": "any",
                "disable_parallel_tool_use": true
            }
        });

        keyboard.progress()?;


        let raw_response = ureq::post("https://api.anthropic.com/v1/messages")
            .set("x-api-key", api_key.as_str())
            .set("anthropic-version", "2023-06-01")
            .set("Content-Type", "application/json")
            .send_json(&body);

        let response = match raw_response {
            Ok(response) => { response }
            Err(Error::Status(code, response)) => {
                println!("Error: {}", code);
                let json: serde_json::Value = response.into_json()?;
                println!("Response: {}", json);
                // return Err(Error::from(code));
                return Err(anyhow::anyhow!("API ERROR"))
            }
            Err(_) => {
                return Err(anyhow::anyhow!("OTHER API ERROR"))
            }
        };

        keyboard.progress()?;

        let json: serde_json::Value = response.into_json()?;
        println!("Response: {}", json);
        let tool_calls = &json["content"];

        if let Some(tool_call) = tool_calls.get(0) {
            keyboard.progress()?;
            let function_name = tool_call["name"].as_str().unwrap();
            let arguments = &tool_call["input"];
            keyboard.progress()?;

            match function_name {
                "draw_text" => {
                    let text = arguments["text"].as_str().unwrap();
                    draw_text(text, &mut keyboard)?;
                }
                "draw_svg" => {
                    let svg_data = arguments["svg"].as_str().unwrap();
                    draw_svg(svg_data, &mut keyboard, &mut pen)?;
                }
                _ => {
                    keyboard.progress_end()?;
                    return Err(anyhow::anyhow!("Unknown function called"))
                }
            }
        } else {
            keyboard.progress_end()?;
            return Err(anyhow::anyhow!("No tool call found in response"))
        }
    }
}
