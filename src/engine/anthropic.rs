use anyhow::Result;
use serde_json::Value as json;
use serde_json::json;
// use ureq::Error;

pub struct Tool {
    name: String,
    definition: json,
    callback: Option<Box<dyn FnMut(json)>>,
}

pub struct Anthropic {
    model: String,
    api_key: String,
    tools: Vec<Tool>,
    content: Vec<json>,
}

impl Anthropic {
    pub fn new(model: String) -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap();
        Self {
            model,
            api_key,
            tools: Vec::new(),
            content: Vec::new(),
        }
    }

    pub fn register_tool(&mut self, name: &str, definition: json, callback: Box<dyn FnMut(json)>) {
        self.tools.push(Tool {
            name: name.to_string(),
            definition,
            callback: Some(callback),
        });
    }

    pub fn add_content(&mut self, content: json) {
        self.content.push(content);
    }

    pub fn add_text_content(&mut self, text: &str) {
        self.add_content(json!({
            "type": "text",
            "text": text,
        }));
    }

    pub fn add_image_content(&mut self, base64_image: &str) {
        self.add_content(json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": base64_image
            }
        }));
    }



    pub fn clear_content(&mut self) {
        self.content.clear();
    }

    fn anthropic_tool_definition(tool: &Tool) -> json {
        json!({
            "name": tool.definition["name"],
            "description": tool.definition["description"],
            "input_schema": tool.definition["parameters"],
        })
    }

    pub fn execute(&mut self) -> Result<()> {

        let body = json!({
            "model": "claude-3-5-sonnet-latest",
            "max_tokens": 5000,
            "messages": [{
                "role": "user",
                "content": self.content
            }],
            "tools": self.tools.iter().map(|tool| Self::anthropic_tool_definition(tool)).collect::<Vec<_>>(),
            "tool_choice": {
                "type": "any",
                "disable_parallel_tool_use": true
            }
        });

        // print body for debugging
        println!("Request: {}", body);


           let response = ureq::post("https://api.anthropic.com/v1/messages")
                    .set("x-api-key", self.api_key.as_str())
                    .set("anthropic-version", "2023-06-01")
                      .set("Content-Type", "application/json")
                    .send_json(&body)
                    .unwrap();

        // let response = match raw_response {
        //     Ok(response) => response,
        //     Err(Error::Status(code, response)) => {
        //         println!("Error: {}", code);
        //         let json: serde_json = response.into_json()?;
        //         println!("Response: {}", json);
        //         return Err(anyhow::anyhow!("API ERROR"));
        //     }
        //     Err(_) => return Err(anyhow::anyhow!("OTHER API ERROR")),
        // };

        let json: json = response.into_json().unwrap();
        println!("Response: {}", json);
        let tool_calls = &json["content"];
        // let tool_calls = &json["choices"][0]["message"]["tool_calls"];
        if let Some(tool_call) = tool_calls.get(0) {
            let function_name = tool_call["name"].as_str().unwrap();
            let function_input = &tool_call["input"];
            // let function_input = serde_json::from_str::<json>(raw_function_input).unwrap();
            let tool = self.tools.iter_mut().find(|tool| tool.name == function_name);
            if let Some(tool) = tool {
                if let Some(callback) = &mut tool.callback {
                    callback(function_input.clone());
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("No callback registered for tool {}", function_name))
                }
            } else {
                Err(anyhow::anyhow!("No tool registered with name {}", function_name))
            }
        } else {
            Err(anyhow::anyhow!("No tool calls found in response"))
        }
    }
}

