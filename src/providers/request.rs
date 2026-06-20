use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRequest {
    pub prompt: String,
    pub model: String,
    pub size: String,
    pub quality: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageResult {
    pub base64: String,
    pub mime_hint: Option<String>,
}

impl ImageRequest {
    pub fn body(&self) -> Value {
        image_request_body(&self.prompt, &self.model, &self.size, &self.quality)
    }
}

pub fn image_request_body(prompt: &str, model: &str, size: &str, quality: &str) -> Value {
    json!({
        "model": model,
        "store": false,
        "stream": true,
        "instructions": "Use the image_generation tool. No text unless image generation is unavailable.",
        "input": [{
            "role": "user",
            "content": prompt,
        }],
        "tools": [{
            "type": "image_generation",
            "action": "generate",
            "quality": quality,
            "size": size,
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
        let body = image_request_body("red circle", "gpt-5.5", "1024x1024", "low");

        assert_eq!(body["model"], "gpt-5.5");
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"], "red circle");
        assert_eq!(body["tools"][0]["type"], "image_generation");
        assert_eq!(body["tools"][0]["action"], "generate");
        assert_eq!(body["tools"][0]["quality"], "low");
        assert_eq!(body["tools"][0]["size"], "1024x1024");
        assert_eq!(body["tools"][0]["partial_images"], 1);
        assert_eq!(body["tool_choice"]["type"], "image_generation");
    }
}
