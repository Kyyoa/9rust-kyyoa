use serde_json::Value;

#[derive(Clone, Copy)]
pub struct RtkEngine;

impl RtkEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn apply(&self, payload: &mut Value, max_tool_chars: usize, compress_whitespace: bool) -> (usize, usize) {
        let before_bytes = payload.to_string().len();

        if let Some(messages) = payload.get_mut("messages").and_then(|m| m.as_array_mut()) {
            for msg in messages.iter_mut() {
                // 1. Tool result truncation (RTK logic)
                let is_tool = msg.get("role").and_then(|r| r.as_str()) == Some("tool");
                if is_tool && max_tool_chars > 0 {
                    if let Some(content) = msg.get_mut("content") {
                        if let Some(text) = content.as_str() {
                            if text.len() > max_tool_chars {
                                let half = max_tool_chars / 2;
                                let head = &text[..half];
                                let tail = &text[text.len() - half..];
                                let truncated = format!(
                                    "{}\n... [9rust RTK: truncated {} chars] ...\n{}",
                                    head,
                                    text.len() - max_tool_chars,
                                    tail
                                );
                                *content = Value::String(truncated);
                            }
                        }
                    }
                }

                // 2. Whitespace compression
                if compress_whitespace {
                    if let Some(content) = msg.get_mut("content") {
                        if let Some(text) = content.as_str() {
                            let compressed: String = text
                                .lines()
                                .map(|l| l.trim_end())
                                .filter(|l| !l.is_empty())
                                .collect::<Vec<_>>()
                                .join("\n");
                            *content = Value::String(compressed);
                        }
                    }
                }
            }
        }

        let after_bytes = payload.to_string().len();
        (before_bytes, after_bytes)
    }
}
