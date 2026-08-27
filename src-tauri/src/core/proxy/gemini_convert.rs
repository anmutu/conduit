//! Anthropic → Gemini 协议转换。
//!
//! 触发场景:Claude Code 等 Anthropic 协议客户端要使用只提供 Gemini 端点的供应商。
//! 路径形态 /v1beta/models/{model}:generateContent(非流式)或
//! :streamGenerateContent?alt=sse(流式);回程统一转回 Anthropic 格式。

use serde_json::{json, Value};

/// Anthropic 请求体 → Gemini 请求体(不含路径,路径需带模型名)。
pub fn request(body: &Value) -> Value {
    let mut out = json!({});
    let mut cfg = json!({});
    if let Some(n) = body.get("max_tokens").and_then(|v| v.as_i64()) {
        cfg["maxOutputTokens"] = json!(n);
    }
    // thinking.budget_tokens → Gemini 2.5 thinkingConfig(预算直传,上限 32k)
    if body.pointer("/thinking/type").and_then(|v| v.as_str()) == Some("enabled") {
        let budget = body
            .pointer("/thinking/budget_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(8_000)
            .clamp(0, 32_767);
        cfg["thinkingConfig"] = json!({ "thinkingBudget": budget, "includeThoughts": true });
    }
    for (src, dst) in [("temperature", "temperature"), ("top_p", "topP")] {
        if let Some(v) = body.get(src) {
            if !v.is_null() {
                cfg[dst] = v.clone();
            }
        }
    }
    if !cfg.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        out["generationConfig"] = cfg;
    }
    if let Some(sys) = body.get("system") {
        let text = match sys {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        if !text.is_empty() {
            out["systemInstruction"] = json!({ "parts": [{ "text": text }] });
        }
    }

    let mut contents: Vec<Value> = Vec::new();
    if let Some(msgs) = body.get("messages").and_then(|v| v.as_array()) {
        for m in msgs {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let grole = if role == "assistant" { "model" } else { "user" };
            let blocks = match m.get("content") {
                Some(Value::Array(a)) => a.clone(),
                Some(Value::String(s)) => vec![json!({ "type": "text", "text": s })],
                _ => continue,
            };
            let mut parts: Vec<Value> = Vec::new();
            for b in &blocks {
                match b.get("type").and_then(|t| t.as_str()) {
                    Some("tool_use") => {
                        let name = b.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        // Gemini 要求 functionCall.args 为 object
                        let args = b
                            .get("input")
                            .and_then(|i| i.as_object().cloned())
                            .unwrap_or_default();
                        parts.push(json!({ "functionCall": { "name": name, "args": args } }));
                    }
                    Some("tool_result") => {
                        let content = match b.get("content") {
                            Some(Value::String(s)) => s.clone(),
                            Some(Value::Array(a)) => a
                                .iter()
                                .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join("\n"),
                            _ => String::new(),
                        };
                        parts.push(json!({
                            "functionResponse": {
                                "name": b.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                                "response": { "result": content }
                            }
                        }));
                    }
                    _ => {
                        if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                            parts.push(json!({ "text": t }));
                        }
                    }
                }
            }
            if !parts.is_empty() {
                contents.push(json!({ "role": grole, "parts": parts }));
            }
        }
    }
    out["contents"] = json!(contents);

    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        let fns: Vec<Value> = tools
            .iter()
            .filter_map(|t| {
                let name = t.get("name")?.as_str()?;
                Some(json!({
                    "name": name,
                    "description": t.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                    "parameters": t.get("input_schema").cloned().unwrap_or(json!({"type":"object"})),
                }))
            })
            .collect();
        if !fns.is_empty() {
            out["tools"] = json!([{ "functionDeclarations": fns }]);
        }
    }
    out
}

/// Gemini 非流式响应 → Anthropic message 响应。
pub fn response(body: &Value) -> Value {
    let cand = body.pointer("/candidates/0").cloned().unwrap_or(json!({}));
    let mut content: Vec<Value> = Vec::new();
    if let Some(parts) = cand.pointer("/content/parts").and_then(|p| p.as_array()) {
        for p in parts {
            if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                if !t.is_empty() {
                    content.push(json!({ "type": "text", "text": t }));
                }
            }
            if let Some(fc) = p.get("functionCall") {
                content.push(json!({
                    "type": "tool_use",
                    "id": format!("toolu_{}", fc.get("name").and_then(|n| n.as_str()).unwrap_or("call")),
                    "name": fc.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                    "input": fc.get("args").cloned().unwrap_or(json!({})),
                }));
            }
        }
    }
    let stop_reason = match cand.get("finishReason").and_then(|f| f.as_str()) {
        Some("STOP") => "end_turn",
        Some("MAX_TOKENS") => "max_tokens",
        Some("SAFETY") | Some("RECITATION") => "refusal",
        _ if content.iter().any(|c| c["type"] == "tool_use") => "tool_use",
        _ => "end_turn",
    };
    json!({
        "id": "msg_gemini",
        "type": "message",
        "role": "assistant",
        "model": body.get("modelVersion").and_then(|m| m.as_str()).unwrap_or(""),
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": body.pointer("/usageMetadata/promptTokenCount").and_then(|v| v.as_i64()).unwrap_or(0),
            "output_tokens": body.pointer("/usageMetadata/candidatesTokenCount").and_then(|v| v.as_i64()).unwrap_or(0),
        }
    })
}

/// Gemini SSE 流(alt=sse,data: {...candidates...})→ Anthropic SSE。
/// 复用 OpenAI 转换器的流式骨架:这里先把每个 Gemini chunk 归一化为
/// OpenAI chunk 形态,再交给同一个 SseConverter 吐 Anthropic 事件。
pub struct GeminiSseConverter {
    inner: super::convert::SseConverter,
    line_buf: String,
}

impl Default for GeminiSseConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiSseConverter {
    pub fn new() -> Self {
        Self {
            inner: super::convert::SseConverter::new(),
            line_buf: String::new(),
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.line_buf.push_str(&String::from_utf8_lossy(chunk));
        let mut lines: Vec<String> = Vec::new();
        while let Some(pos) = self.line_buf.find('\n') {
            let line = self.line_buf[..pos].trim_end_matches('\r').to_string();
            self.line_buf.replace_range(..pos + 1, "");
            lines.push(line);
        }
        let mut out = Vec::new();
        for line in lines {
            if !line.starts_with("data:") {
                continue;
            }
            let data = line[5..].trim();
            if data == "[DONE]" {
                // Gemini 正常不发 [DONE];个别网关会补,映射成 stop 收尾
                out.extend_from_slice(&self.inner.feed(
                    br#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}

data: [DONE]

"#,
                ));
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            let oai = gemini_chunk_to_openai(&v);
            let payload = format!("data: {}\n\n", oai);
            out.extend_from_slice(&self.inner.feed(payload.as_bytes()));
        }
        out
    }

    /// 上游断流兜底:补齐终止事件
    pub fn finish(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        // 喂一个带 finish_reason 的哨兵块触发收尾
        let fin = self.inner.feed(
            br#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}

data: [DONE]

"#,
        );
        out.extend_from_slice(&fin);
        out
    }
}

/// 单个 Gemini 流式 chunk → OpenAI chunk 形态(供 SseConverter 消费)
fn gemini_chunk_to_openai(v: &Value) -> Value {
    let mut oai = json!({ "choices": [{}] });
    if let Some(id) = v.get("candidates") {
        // id/model 字段仅首个 chunk 需要
        oai["id"] = json!("gemini_stream");
        oai["model"] = json!(v.get("modelVersion").and_then(|m| m.as_str()).unwrap_or(""));
        let _ = id;
    }
    let cand = v.pointer("/candidates/0").cloned().unwrap_or(json!({}));
    let mut delta = json!({});
    if let Some(parts) = cand.pointer("/content/parts").and_then(|p| p.as_array()) {
        let mut text = String::new();
        let mut tool_calls: Vec<Value> = Vec::new();
        for p in parts {
            if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                text.push_str(t);
            }
            if let Some(fc) = p.get("functionCall") {
                tool_calls.push(json!({
                    "index": tool_calls.len(),
                    "id": format!("call_{}", tool_calls.len()),
                    "function": {
                        "name": fc.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                        "arguments": fc.get("args").cloned().unwrap_or(json!({})).to_string(),
                    }
                }));
            }
        }
        if !text.is_empty() {
            delta["content"] = json!(text);
        }
        if !tool_calls.is_empty() {
            delta["tool_calls"] = json!(tool_calls);
        }
    }
    let finish = match cand.get("finishReason").and_then(|f| f.as_str()) {
        Some("STOP") => Some("stop"),
        Some("MAX_TOKENS") => Some("length"),
        Some("SAFETY") | Some("RECITATION") => Some("content_filter"),
        _ => None,
    };
    oai["choices"][0]["delta"] = delta;
    if let Some(f) = finish {
        oai["choices"][0]["finish_reason"] = json!(f);
    }
    if let Some(u) = v.get("usageMetadata") {
        oai["usage"] = json!({
            "prompt_tokens": u.get("promptTokenCount").and_then(|x| x.as_i64()).unwrap_or(0),
            "completion_tokens": u.get("candidatesTokenCount").and_then(|x| x.as_i64()).unwrap_or(0),
        });
    }
    oai
}

/// 流式转换器:上游 Gemini SSE 流 → Anthropic SSE 流(断流自动补齐)
pub struct GeminiConvertStream<S> {
    inner: S,
    conv: GeminiSseConverter,
    done: bool,
}

impl<S> GeminiConvertStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            conv: GeminiSseConverter::new(),
            done: false,
        }
    }
}

impl<S, E> futures::Stream for GeminiConvertStream<S>
where
    S: futures::Stream<Item = Result<bytes::Bytes, E>> + Unpin,
    E: std::fmt::Display,
{
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::pin::Pin;
        use std::task::Poll;
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                let out = self.conv.feed(&chunk);
                if out.is_empty() {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Poll::Ready(Some(Ok(bytes::Bytes::from(out))))
            }
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Some(Err(std::io::Error::other(e.to_string()))))
            }
            Poll::Ready(None) => {
                if self.done {
                    return Poll::Ready(None);
                }
                self.done = true;
                let out = self.conv.finish();
                if out.is_empty() {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(bytes::Bytes::from(out))))
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_maps_basic() {
        let body = json!({
            "model": "gemini-x", "max_tokens": 99, "temperature": 0.5,
            "system": "brief",
            "messages": [
                { "role": "user", "content": "hi" },
                { "role": "assistant", "content": [
                    { "type": "tool_use", "id": "t1", "name": "ls", "input": {"path": "/"} }
                ]},
                { "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": "t1", "name": "ls", "content": "ok" }
                ]}
            ],
            "tools": [{ "name": "ls", "description": "d", "input_schema": {"type":"object"} }],
        });
        let out = request(&body);
        assert_eq!(out["systemInstruction"]["parts"][0]["text"], "brief");
        assert_eq!(out["contents"][0]["role"], "user");
        assert_eq!(out["contents"][1]["role"], "model");
        assert_eq!(out["contents"][1]["parts"][0]["functionCall"]["name"], "ls");
        assert_eq!(
            out["contents"][2]["parts"][0]["functionResponse"]["name"],
            "ls"
        );
        assert_eq!(out["generationConfig"]["maxOutputTokens"], 99);
        assert_eq!(out["tools"][0]["functionDeclarations"][0]["name"], "ls");
    }

    #[test]
    fn resp_maps_tool_call() {
        let body = json!({
            "modelVersion": "gemini-x",
            "candidates": [{ "finishReason": "STOP", "content": { "parts": [
                { "text": "checking" },
                { "functionCall": { "name": "ls", "args": {"path": "/tmp"} } }
            ]}}],
            "usageMetadata": { "promptTokenCount": 4, "candidatesTokenCount": 6 }
        });
        let out = response(&body);
        assert_eq!(out["content"][0]["type"], "text");
        assert_eq!(out["content"][1]["type"], "tool_use");
        assert_eq!(out["content"][1]["input"]["path"], "/tmp");
        assert_eq!(out["usage"]["input_tokens"], 4);
        assert_eq!(out["usage"]["output_tokens"], 6);
    }

    #[test]
    fn sse_text_flow() {
        let mut c = GeminiSseConverter::new();
        let out = c.feed(
            br#"data: {"candidates":[{"content":{"parts":[{"text":"Hel"}]}}]}

"#,
        );
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("event: message_start"), "{s}");
        assert!(s.contains("text_delta"), "{s}");
        let out = c.feed(br#"data: {"candidates":[{"content":{"parts":[{"text":"lo"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":2}}

"#);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\"stop_reason\":\"end_turn\""), "{s}");
        assert!(s.contains("event: message_stop"), "{s}");
    }

    #[test]
    fn sse_tool_flow() {
        let mut c = GeminiSseConverter::new();
        let out = c.feed(br#"data: {"candidates":[{"content":{"parts":[{"functionCall":{"name":"ls","args":{"path":"/"}}}]}}]}

"#);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("content_block_start"), "{s}");
        assert!(s.contains("tool_use"), "{s}");
        let out = c.feed(
            br#"data: {"candidates":[{"content":{"parts":[]},"finishReason":"STOP"}]}

"#,
        );
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("input_json_delta"), "{s}");
    }
    #[test]
    fn request_maps_thinking_to_thinking_config() {
        let out = request(&json!({
            "model": "gemini-2.5-pro", "max_tokens": 1024,
            "thinking": { "type": "enabled", "budget_tokens": 10000 },
            "messages": [{ "role": "user", "content": "hi" }]
        }));
        assert_eq!(
            out["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            10000
        );
        assert_eq!(
            out["generationConfig"]["thinkingConfig"]["includeThoughts"],
            true
        );
        // 未开启 thinking 时不注入
        let plain = request(&json!({
            "model": "gemini-2.5-flash", "max_tokens": 64,
            "messages": [{ "role": "user", "content": "hi" }]
        }));
        assert!(plain["generationConfig"].get("thinkingConfig").is_none());
    }
}
