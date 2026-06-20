use serde_json::{Value, json};

const SYSTEM_PROMPT: &str = include_str!("../../prompts/system.md");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRequest {
    pub prompt: String,
    pub model: String,
    pub size: String,
    pub quality: String,
    pub output_format: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageResult {
    pub base64: String,
    pub mime_hint: Option<String>,
}

impl ImageRequest {
    pub fn body(&self) -> Value {
        image_request_body(
            &self.prompt,
            &self.model,
            &self.size,
            &self.quality,
            &self.output_format,
        )
    }
}

pub fn image_request_body(
    prompt: &str,
    model: &str,
    size: &str,
    quality: &str,
    output_format: &str,
) -> Value {
    json!({
        "model": model,
        "store": false,
        "stream": true,
        "instructions": SYSTEM_PROMPT,
        "input": [{
            "role": "user",
            "content": prompt,
        }],
        "tools": [{
            "type": "image_generation",
            "action": "generate",
            "quality": quality,
            "size": size,
            "output_format": output_format,
            "partial_images": 1,
        }],
        "tool_choice": { "type": "image_generation" },
        "text": { "verbosity": "low" }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_builder_matches_codex_image_shape() {
        let body = image_request_body("red circle", "gpt-5.4", "1024x1024", "low", "png");

        assert_eq!(body["model"], "gpt-5.4");
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"], "red circle");
        assert_eq!(body["tools"][0]["type"], "image_generation");
        assert_eq!(body["tools"][0]["action"], "generate");
        assert_eq!(body["tools"][0]["quality"], "low");
        assert_eq!(body["tools"][0]["size"], "1024x1024");
        assert_eq!(body["tools"][0]["output_format"], "png");
        assert_eq!(body["tools"][0]["partial_images"], 1);
        assert_eq!(body["tool_choice"]["type"], "image_generation");
    }

    #[test]
    fn output_format_webp_is_written_to_tool_body() {
        let body = image_request_body("red circle", "gpt-5.4", "1024x1024", "low", "webp");

        assert_eq!(body["tools"][0]["output_format"], "webp");
    }

    #[test]
    fn image_request_default_output_format_is_png() {
        let request = ImageRequest {
            prompt: "red circle".to_string(),
            model: "gpt-5.4".to_string(),
            size: "1024x1024".to_string(),
            quality: "low".to_string(),
            output_format: "png".to_string(),
        };

        assert_eq!(request.body()["tools"][0]["output_format"], "png");
    }

    #[test]
    fn system_prompt_include_loads_non_empty_content() {
        assert!(!SYSTEM_PROMPT.trim().is_empty());
        assert_eq!(
            SYSTEM_PROMPT.trim(),
            "Use the image_generation tool. No text unless image generation is unavailable."
        );
    }
}
