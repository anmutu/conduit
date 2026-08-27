//! OpenAI → Anthropic 协议转换(反向)。
//!
//! 触发场景:Codex 等 OpenAI 协议客户端(chat/completions)要使用只有
//! Anthropic 端点的供应商。请求转成 /v1/messages 形态,回程转回 OpenAI
//! 格式(含 SSE 流与 tool_calls 增量)。/v1/responses 路径不在转换范围(透传)。

use serde_json::{json, Value};

/// OpenAI chat/completions 请求体 → Anthropic messages 请求体
pub fn request(body: &Value) -> Value {
    let mut out = json!({});
    if let Some(m) = body.get("model").and_then(|v| v.as_str()) {
        out["model"] = json!(m);
    }
    // Anthropic 必填 max_tokens
    out["max_tokens"] = json!(body
        .get("max_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(8192));
    for k in ["temperature", "top_p"] {
        if let Some(v) = body.get(k) {
            if !v.is_null() {
                out[k] = v.clone();
            }
        }
    }
    if body.get("stream").and_then(|v| v.as_bool()) == Some(true) {
        out["stream"] = json!(true);
    }

    let mut messages: Vec<Value> = Vec::new();
    if let Some(msgs) = body.get("messages").and_then(|v| v.as_array()) {
        for m in msgs {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            match role {
                "system" | "developer" => {
                    let text = content_to_text(m.get("content"));
                    if !text.is_empty() {
                        // 多条 system 合并为一个 system 字段
                        if let Some(prev) = out.get("system").and_then(|s| s.as_str()) {
                            out["system"] = json!(format!("{prev}\n{text}"));
                        } else {
                            out["system"] = json!(text);
                        }
                    }
                }
                "tool" => {
                    let call_id = m.get("tool_call_id").and_then(|i| i.as_str()).unwrap_or("");
                    let content = content_to_text(m.get("content"));
                    messages.push(json!({
                        "role": "user",
                        "content": [{ "type": "tool_result", "tool_use_id": call_id, "content": content }]
                    }));
                }
                "assistant" => {
                    // tool_calls → tool_use blocks
                    let mut blocks: Vec<Value> = Vec::new();
                    if let Some(text) = m.get("content") {
                        let t = content_to_text(Some(text));
                        if !t.is_empty() {
                            blocks.push(json!({ "type": "text", "text": t }));
                        }
                    }
                    if let Some(calls) = m.get("tool_calls").and_then(|c| c.as_array()) {
                        for c in calls {
                            let args = c
                                .pointer("/function/arguments")
                                .and_then(|a| a.as_str())
                                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                                .unwrap_or(json!({}));
                            blocks.push(json!({
                                "type": "tool_use",
                                "id": c.get("id").and_then(|i| i.as_str()).unwrap_or("toolu_missing"),
                                "name": c.pointer("/function/name").and_then(|n| n.as_str()).unwrap_or(""),
                                "input": args,
                            }));
                        }
                    }
                    if !blocks.is_empty() {
                        messages.push(json!({ "role": "assistant", "content": blocks }));
                    }
                }
                _ => {
                    let text = content_to_text(m.get("content"));
                    if !text.is_empty() {
                        messages.push(json!({ "role": "user", "content": text }));
                    }
                }
            }
        }
    }
    out["messages"] = json!(messages);

    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        let fns: Vec<Value> = tools
            .iter()
            .filter_map(|t| {
                let name = t.pointer("/function/name")?.as_str()?;
                Some(json!({
                    "name": name,
                    "description": t.pointer("/function/description").and_then(|d| d.as_str()).unwrap_or(""),
                    "input_schema": t.pointer("/function/parameters").cloned().unwrap_or(json!({"type":"object"})),
                }))
            })
            .collect();
        if !fns.is_empty() {
            out["tools"] = json!(fns);
            out["tool_choice"] = match body.get("tool_choice").and_then(|c| c.as_str()) {
                Some("required") => json!({ "type": "any" }),
                Some("none") => json!({ "type": "none" }),
                _ => json!({ "type": "auto" }),
            };
        }
    }
    out
}

/// OpenAI content(字符串或分段数组)→ 纯文本
fn content_to_text(c: Option<&Value>) -> String {
    match c {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Anthropic message 响应 → OpenAI chat completion 响应
pub fn response(body: &Value) -> Value {
    let mut text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    if let Some(blocks) = body.get("content").and_then(|c| c.as_array()) {
        for b in blocks {
            match b.get("type").and_then(|t| t.as_str()) {
                Some("text") => text.push_str(b.get("text").and_then(|t| t.as_str()).unwrap_or("")),
                Some("tool_use") => tool_calls.push(json!({
                    "id": b.get("id").and_then(|i| i.as_str()).unwrap_or("call_missing"),
                    "type": "function",
                    "function": {
                        "name": b.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                        "arguments": b.get("input").cloned().unwrap_or(json!({})).to_string(),
                    }
                })),
                _ => {}
            }
        }
    }
    let finish = match body.get("stop_reason").and_then(|s| s.as_str()) {
        Some("tool_use") => "tool_calls",
        Some("max_tokens") => "length",
        _ => "stop",
    };
    let mut message = json!({ "role": "assistant" });
    if !text.is_empty() {
        message["content"] = json!(text);
    }
    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls);
    }
    json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or("chatcmpl_conv"),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "choices": [{ "index": 0, "message": message, "finish_reason": finish }],
        "usage": {
            "prompt_tokens": body.pointer("/usage/input_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
            "completion_tokens": body.pointer("/usage/output_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        }
    })
}

// ============ SSE 流:Anthropic events → OpenAI chunks ============

/// 有状态转换器:Anthropic SSE 字节流 → OpenAI SSE 字节流。
pub struct AnthropicSseConverter {
    line_buf: String,
    /// block index → tool 槽位(openai tool_calls 数组下标)
    tool_slots: std::collections::HashMap<i64, usize>,
    next_slot: usize,
    sent_role: bool,
    sent_done: bool,
}

impl Default for AnthropicSseConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl AnthropicSseConverter {
    pub fn new() -> Self {
        Self {
            line_buf: String::new(),
            tool_slots: std::collections::HashMap::new(),
            next_slot: 0,
            sent_role: false,
            sent_done: false,
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
                continue; // Anthropic 上游不会有;若有则忽略
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            self.on_event(&v, &mut out);
        }
        out
    }

    fn on_event(&mut self, v: &Value, out: &mut Vec<u8>) {
        let ev = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if !self.sent_role && (ev == "message_start" || ev == "content_block_start") {
            self.sent_role = true;
            self.emit_chunk(
                out,
                json!({ "delta": { "role": "assistant", "content": "" } }),
            );
        }
        match ev {
            "content_block_start" => {
                let idx = v.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
                if v.pointer("/content_block/type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let slot = self.next_slot;
                    self.next_slot += 1;
                    self.tool_slots.insert(idx, slot);
                    self.emit_chunk(
                        out,
                        json!({ "delta": { "tool_calls": [{
                            "index": slot,
                            "id": v.pointer("/content_block/id").and_then(|x| x.as_str()).unwrap_or(""),
                            "type": "function",
                            "function": {
                                "name": v.pointer("/content_block/name").and_then(|x| x.as_str()).unwrap_or(""),
                                "arguments": "",
                            }
                        }] } }),
                    );
                }
            }
            "content_block_delta" => {
                let idx = v.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
                match v.pointer("/delta/type").and_then(|t| t.as_str()) {
                    Some("text_delta") => {
                        let t = v
                            .pointer("/delta/text")
                            .and_then(|x| x.as_str())
                            .unwrap_or("");
                        if !t.is_empty() {
                            self.emit_chunk(out, json!({ "delta": { "content": t } }));
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(&slot) = self.tool_slots.get(&idx) {
                            let p = v
                                .pointer("/delta/partial_json")
                                .cloned()
                                .unwrap_or(json!(""));
                            self.emit_chunk(
                                out,
                                json!({ "delta": { "tool_calls": [{
                                    "index": slot,
                                    "function": { "arguments": p }
                                }] } }),
                            );
                        }
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                let finish = match v.pointer("/delta/stop_reason").and_then(|s| s.as_str()) {
                    Some("tool_use") => "tool_calls",
                    Some("max_tokens") => "length",
                    _ => "stop",
                };
                let mut chunk = json!({ "delta": {}, "finish_reason": finish });
                if let Some(u) = v.get("usage") {
                    chunk["usage"] = json!({
                        "prompt_tokens": 0,
                        "completion_tokens": u.get("output_tokens").and_then(|x| x.as_i64()).unwrap_or(0),
                    });
                }
                self.emit_chunk(out, chunk);
            }
            "message_stop" if !self.sent_done => {
                self.sent_done = true;
                out.extend_from_slice(b"data: [DONE]\n\n");
            }
            _ => {}
        }
    }

    /// 上游断流兜底:补 finish + [DONE]
    pub fn finish(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        if self.sent_role && !self.sent_done {
            self.emit_chunk(&mut out, json!({ "delta": {}, "finish_reason": "stop" }));
            out.extend_from_slice(b"data: [DONE]\n\n");
            self.sent_done = true;
        }
        out
    }

    fn emit_chunk(&self, out: &mut Vec<u8>, delta: Value) {
        let chunk = json!({ "id": "chatcmpl_conv", "object": "chat.completion.chunk",
                            "model": "", "choices": [{ "index": 0, "delta": delta }] });
        out.extend_from_slice(format!("data: {chunk}\n\n").as_bytes());
    }
}

/// 流式转换器:上游 Anthropic SSE 流 → OpenAI SSE 流(断流自动补齐)
pub struct AnthropicConvertStream<S> {
    inner: S,
    conv: AnthropicSseConverter,
    done: bool,
}

impl<S> AnthropicConvertStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            conv: AnthropicSseConverter::new(),
            done: false,
        }
    }
}

impl<S, E> futures::Stream for AnthropicConvertStream<S>
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
            "model": "claude-x", "max_tokens": 55,
            "messages": [
                { "role": "system", "content": "brief" },
                { "role": "user", "content": "hi" },
                { "role": "assistant", "content": "checking", "tool_calls": [
                    { "id": "c1", "function": { "name": "ls", "arguments": "{\"path\":\"/\"}" } }
                ]},
                { "role": "tool", "tool_call_id": "c1", "content": "ok" }
            ],
            "tools": [{ "type": "function", "function": { "name": "ls", "description": "d", "parameters": {"type":"object"} } }],
            "tool_choice": "auto",
        });
        let out = request(&body);
        assert_eq!(out["system"], "brief");
        assert_eq!(out["messages"][0]["content"], "hi");
        let asst = &out["messages"][1];
        assert_eq!(asst["content"][0]["type"], "text");
        assert_eq!(asst["content"][1]["type"], "tool_use");
        assert_eq!(asst["content"][1]["input"]["path"], "/");
        assert_eq!(out["messages"][2]["content"][0]["type"], "tool_result");
        assert_eq!(out["tools"][0]["name"], "ls");
        assert_eq!(out["tool_choice"]["type"], "auto");
    }

    #[test]
    fn resp_maps_tool_use() {
        let body = json!({
            "id": "msg_1", "model": "claude-x",
            "content": [
                { "type": "text", "text": "let me check" },
                { "type": "tool_use", "id": "t1", "name": "ls", "input": {"path": "/tmp"} }
            ],
            "stop_reason": "tool_use",
            "usage": { "input_tokens": 5, "output_tokens": 3 }
        });
        let out = response(&body);
        assert_eq!(out["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(out["choices"][0]["message"]["content"], "let me check");
        assert_eq!(
            out["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "ls"
        );
        assert_eq!(out["usage"]["prompt_tokens"], 5);
    }

    #[test]
    fn sse_round_flow() {
        let mut c = AnthropicSseConverter::new();
        let out = c.feed(
            br#"event: message_start
data: {"type":"message_start","message":{"usage":{"input_tokens":5,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}

"#,
        );
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\"role\":\"assistant\""), "{s}");
        assert!(s.contains("\"content\":\"Hi\""), "{s}");

        let out = c.feed(br#"event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t1","name":"ls","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"a\":1}"}}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":9}}

event: message_stop
data: {"type":"message_stop"}

"#);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\"tool_calls\""), "{s}");
        assert!(s.contains("\"name\":\"ls\""), "{s}");
        assert!(s.contains("\"arguments\":\"{\\\"a\\\":1}\""), "{s}");
        assert!(s.contains("\"finish_reason\":\"tool_calls\""), "{s}");
        assert!(s.contains("data: [DONE]"), "{s}");
    }
}
