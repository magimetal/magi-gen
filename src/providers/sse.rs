use crate::providers::request::ImageResult;
use anyhow::Context;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct SseImageParser {
    buffer: String,
    partial: Option<String>,
    final_image: Option<String>,
    text: Vec<String>,
    event_types: Vec<String>,
    done: bool,
}

impl SseImageParser {
    pub fn push_chunk(&mut self, chunk: &str) -> anyhow::Result<()> {
        self.buffer.push_str(chunk);
        while let Some(index) = self.buffer.find("\n\n") {
            let event = self.buffer[..index].to_string();
            self.buffer.drain(..index + 2);
            self.process_event(&event)?;
            if self.done {
                break;
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> anyhow::Result<ImageResult> {
        if !self.buffer.trim().is_empty() {
            let event = std::mem::take(&mut self.buffer);
            self.process_event(&event)?;
        }
        let base64 = self.final_image.or(self.partial).ok_or_else(|| {
            anyhow::anyhow!(
                "no image result in provider stream; event types: {}; text: {}",
                self.event_types.join(","),
                self.text.join(" ")
            )
        })?;
        Ok(ImageResult {
            base64,
            mime_hint: Some("image/png".to_string()),
        })
    }

    fn process_event(&mut self, event: &str) -> anyhow::Result<()> {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            return Ok(());
        }
        if data.trim() == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let value: Value =
            serde_json::from_str(&data).context("malformed provider SSE data JSON")?;
        self.process_value(&value);
        Ok(())
    }

    fn process_value(&mut self, value: &Value) {
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        self.event_types.push(event_type.to_string());
        match event_type {
            "response.image_generation_call.partial_image" => {
                if let Some(partial) = value.get("partial_image_b64").and_then(Value::as_str) {
                    self.partial = Some(partial.to_string());
                }
            }
            "response.output_item.done" => {
                let item = &value["item"];
                if item.get("type").and_then(Value::as_str) == Some("image_generation_call")
                    && let Some(result) = item.get("result").and_then(Value::as_str)
                {
                    self.final_image = Some(result.to_string());
                }
            }
            "response.completed" => {
                if let Some(output) = value["response"]["output"].as_array() {
                    for item in output {
                        if item.get("type").and_then(Value::as_str) == Some("image_generation_call")
                            && let Some(result) = item.get("result").and_then(Value::as_str)
                        {
                            self.final_image = Some(result.to_string());
                        }
                    }
                }
                self.done = true;
            }
            "response.output_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                    self.text.push(delta.to_string());
                }
            }
            "response.output_text.done" => {
                if let Some(text) = value.get("text").and_then(Value::as_str) {
                    self.text.push(text.to_string());
                }
            }
            _ => {
                if let Some(text) = value.get("text").and_then(Value::as_str) {
                    self.text.push(text.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
pub fn parse_sse_image(input: &str) -> anyhow::Result<ImageResult> {
    let mut parser = SseImageParser::default();
    parser.push_chunk(input)?;
    parser.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_result_beats_partial_result() {
        let result = parse_sse_image(concat!(
            "data: {\"type\":\"response.image_generation_call.partial_image\",\"partial_image_b64\":\"partial\"}\n\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"image_generation_call\",\"status\":\"generating\",\"result\":\"final\"}}\n\n",
            "data: [DONE]\n\n"
        ))
        .unwrap();

        assert_eq!(result.base64, "final");
    }

    #[test]
    fn completed_result_beats_partial_result() {
        let result = parse_sse_image(concat!(
            "data: {\"type\":\"response.image_generation_call.partial_image\",\"partial_image_b64\":\"partial\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"output\":[{\"type\":\"image_generation_call\",\"result\":\"completed\"}]}}\n\n"
        ))
        .unwrap();

        assert_eq!(result.base64, "completed");
    }

    #[test]
    fn partial_result_accepted_when_final_missing() {
        let result = parse_sse_image(concat!(
            "data: {\"type\":\"response.image_generation_call.partial_image\",\"partial_image_b64\":\"partial\"}\n\n",
            "data: [DONE]\n\n"
        ))
        .unwrap();

        assert_eq!(result.base64, "partial");
    }

    #[test]
    fn no_image_returns_text_diagnostic() {
        let error = parse_sse_image(concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"cannot generate\"}\n\n",
            "data: [DONE]\n\n"
        ))
        .unwrap_err()
        .to_string();

        assert!(error.contains("no image result"), "{error}");
        assert!(error.contains("response.output_text.delta"), "{error}");
        assert!(error.contains("cannot generate"), "{error}");
    }

    #[test]
    fn malformed_data_line_is_reported() {
        let error = parse_sse_image("data: {bad}\n\n").unwrap_err().to_string();

        assert!(
            error.contains("malformed provider SSE data JSON"),
            "{error}"
        );
    }
}
