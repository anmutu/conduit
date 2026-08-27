//! Gemini → OpenAI 协议转换(客户端侧)。
//!
//! 触发场景:Gemini CLI 等 Gemini 协议客户端要使用只提供 OpenAI 兼容端点的供应商。
//! 请求形态 /v1beta/models/{model}:generateContent(非流式)或
//! :streamGenerateContent?alt=sse(流式);回程转回 Gemini 格式。

use serde_json::{json, Value};

/// Gemini 请求体 → OpenAI chat/completions 请求体。model 由路径提取,外部注入。
pub fn request(body: &Value, model: &str) -> Value {
    let mut msgs: Vec<Value> = Vec::new();

    if let Some(sys) = body
        .pointer("/systemInstruction/parts")
        .and_then(|p| p.as_array())
    {
        let text = sys
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            msgs.push(json!({ "role": "system", "content": text }));
        }
    }

    if let Some(contents) = body.get("contents").and_then(|c| c.as_array()) {
        for c in contents {
            let role = c.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let is_model = role == "model";
            let parts = c
                .get("parts")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default();
            // functionResponse part 单独成一条 tool 消息;其余合并进正文
            let mut text = String::new();
            let mut tool_calls: Vec<Value> = Vec::new();
            for p in &parts {
                if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
                if let Some(fc) = p.get("functionCall") {
                    tool_calls.push(json!({
                        "id": format!("call_{}", tool_calls.len()),
                        "type": "function",
                        "function": {
                            "name": fc.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                            "arguments": fc.get("args").cloned().unwrap_or(json!({})).to_string(),
                        }
                    }));
                }
                if let Some(fr) = p.get("functionResponse") {
                    let resp = fr.get("response").cloned().unwrap_or(json!({}));
                    let content = match &resp {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    msgs.push(json!({
                        "role": "tool",
                        "tool_call_id": format!("call_{}", fr.get("name").and_then(|n| n.as_str()).unwrap_or("x")),
                        "content": content,
                    }));
                }
            }
            if !text.is_empty() || !tool_calls.is_empty() {
                let mut m = json!({});
                m["role"] = json!(if is_model { "assistant" } else { "user" });
                m["content"] = if text.is_empty() {
                    Value::Null
                } else {
                    json!(text)
                };
                if !tool_calls.is_empty() {
                    m["tool_calls"] = json!(tool_calls);
                }
                msgs.push(m);
            }
        }
    }

    let mut out = json!({ "model": model, "messages": msgs });

    if let Some(cfg) = body.get("generationConfig").filter(|c| c.is_object()) {
        if let Some(n) = cfg.get("maxOutputTokens").and_then(|v| v.as_i64()) {
            out["max_tokens"] = json!(n);
        }
        for (src, dst) in [("temperature", "temperature"), ("topP", "top_p")] {
            if let Some(v) = cfg.get(src) {
                if !v.is_null() {
                    out[dst] = v.clone();
                }
            }
        }
    }

    if let Some(fns) = body
        .pointer("/tools/0/functionDeclarations")
        .and_then(|f| f.as_array())
    {
        let tools: Vec<Value> = fns
            .iter()
            .filter_map(|f| {
                let name = f.get("name")?.as_str()?;
                Some(json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": f.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                        "parameters": f.get("parameters").cloned().unwrap_or(json!({"type":"object"})),
                    }
                }))
            })
            .collect();
        if !tools.is_empty() {
            out["tools"] = json!(tools);
        }
    }
    out
}

/// OpenAI 非流式响应 → Gemini generateContent 响应。
pub fn response(body: &Value) -> Value {
    let msg = body
        .pointer("/choices/0/message")
        .cloned()
        .unwrap_or(json!({}));
    let mut parts: Vec<Value> = Vec::new();
    if let Some(t) = msg.get("content").and_then(|c| c.as_str()) {
        if !t.is_empty() {
            parts.push(json!({ "text": t }));
        }
    }
    let mut has_tool = false;
    if let Some(calls) = msg.get("tool_calls").and_then(|c| c.as_array()) {
        for c in calls {
            let name = c
                .pointer("/function/name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let args = c
                .pointer("/function/arguments")
                .and_then(|a| a.as_str())
                .and_then(|a| serde_json::from_str::<Value>(a).ok())
                .unwrap_or(json!({}));
            if !name.is_empty() {
                parts.push(json!({ "functionCall": { "name": name, "args": args } }));
                has_tool = true;
            }
        }
    }
    if parts.is_empty() {
        parts.push(json!({ "text": "" }));
    }
    let finish = match body
        .pointer("/choices/0/finish_reason")
        .and_then(|f| f.as_str())
    {
        Some("length") => "MAX_TOKENS",
        Some("content_filter") => "SAFETY",
        _ => "STOP",
    };
    let _ = has_tool;
    json!({
        "candidates": [{
            "content": { "role": "model", "parts": parts },
            "finishReason": finish,
            "index": 0,
        }],
        "usageMetadata": {
            "promptTokenCount": body.pointer("/usage/prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
            "candidatesTokenCount": body.pointer("/usage/completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
            "totalTokenCount": (body.pointer("/usage/prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0)
                + body.pointer("/usage/completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0)),
        },
        "modelVersion": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
    })
}

/// OpenAI SSE 流 → Gemini SSE 流(alt=sse 形态,data: {...candidates...})。
/// 纯文本工具调用聚合不做跨块拼接:每个 tool_call delta 以完整 arguments 增量透传,
/// Gemini 的 functionCall.args 为对象,故需聚合到块结束再发(OpenAI 每个 delta
/// 自带增量;此处按 tool_call 为单位在收到 finish/结束时统一输出)。
pub struct GeminiClientSseConverter {
    line_buf: String,
    /// 聚合中的 tool_calls:index -> (name, arguments 字符串)
    tools: Vec<(String, String)>,
    /// 是否已发出带 finishReason 的收尾块
    finished: bool,
}

impl Default for GeminiClientSseConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiClientSseConverter {
    pub fn new() -> Self {
        Self {
            line_buf: String::new(),
            tools: Vec::new(),
            finished: false,
        }
    }

    fn emit(&self, v: &Value) -> Vec<u8> {
        format!("data: {}\n\n", v).into_bytes()
    }

    fn text_chunk(&mut self, text: &str) -> Vec<u8> {
        self.emit(&json!({
            "candidates": [{ "content": { "role": "model", "parts": [{ "text": text }] }, "index": 0 }]
        }))
    }

    fn usage_chunk(&mut self, usage: Option<&Value>, finish: &str) -> Vec<u8> {
        let mut chunk = json!({
            "candidates": [{ "content": { "role": "model", "parts": [] }, "finishReason": finish, "index": 0 }]
        });
        // 聚合完成的 tool_calls 在收尾块输出
        let cand_parts = chunk
            .pointer_mut("/candidates/0/content/parts")
            .and_then(|p| p.as_array_mut())
            .unwrap();
        for (name, args) in self.tools.drain(..) {
            let args = serde_json::from_str::<Value>(&args).unwrap_or(json!({}));
            cand_parts.push(json!({ "functionCall": { "name": name, "args": args } }));
        }
        if let Some(u) = usage {
            chunk["usageMetadata"] = json!({
                "promptTokenCount": u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
                "candidatesTokenCount": u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
            });
        }
        self.finished = true;
        self.emit(&chunk)
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
                // [DONE] 不透传;若尚未发过收尾块,补一个
                if !self.finished {
                    out.extend_from_slice(&self.usage_chunk(None, "STOP"));
                }
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            let choice = v.pointer("/choices/0").cloned().unwrap_or(json!({}));
            let delta = choice.get("delta").cloned().unwrap_or(json!({}));
            if let Some(t) = delta.get("content").and_then(|c| c.as_str()) {
                if !t.is_empty() {
                    out.extend_from_slice(&self.text_chunk(t));
                }
            }
            if let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
                for c in calls {
                    let idx = c.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                    while self.tools.len() <= idx {
                        self.tools.push((String::new(), String::new()));
                    }
                    if let Some(n) = c.pointer("/function/name").and_then(|n| n.as_str()) {
                        self.tools[idx].0 = n.to_string();
                    }
                    if let Some(a) = c.pointer("/function/arguments").and_then(|a| a.as_str()) {
                        self.tools[idx].1.push_str(a);
                    }
                }
            }
            match choice.get("finish_reason").and_then(|f| f.as_str()) {
                Some("length") => {
                    out.extend_from_slice(&self.usage_chunk(v.get("usage"), "MAX_TOKENS"));
                }
                Some(_) => {
                    out.extend_from_slice(&self.usage_chunk(v.get("usage"), "STOP"));
                }
                None => {}
            }
        }
        out
    }

    /// 上游断流兜底:补收尾块
    pub fn finish(&mut self) -> Vec<u8> {
        if self.finished {
            return Vec::new();
        }
        self.usage_chunk(None, "STOP")
    }
}

/// 流式转换器:上游 OpenAI SSE → 下游 Gemini SSE(断流自动补齐)
pub struct GeminiClientConvertStream<S> {
    inner: S,
    conv: GeminiClientSseConverter,
    done: bool,
}

impl<S> GeminiClientConvertStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            conv: GeminiClientSseConverter::new(),
            done: false,
        }
    }
}

impl<S, E> futures::Stream for GeminiClientConvertStream<S>
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
            "systemInstruction": { "parts": [{ "text": "brief" }] },
            "contents": [
                { "role": "user", "parts": [{ "text": "hi" }] },
                { "role": "model", "parts": [{ "functionCall": { "name": "ls", "args": {"path": "/"} } }] },
                { "role": "user", "parts": [{ "functionResponse": { "name": "ls", "response": {"result": "ok"} } }] }
            ],
            "tools": [{ "functionDeclarations": [
                { "name": "ls", "description": "d", "parameters": {"type":"object"} }
            ]}],
            "generationConfig": { "maxOutputTokens": 99, "temperature": 0.5 }
        });
        let out = request(&body, "gpt-test");
        assert_eq!(out["model"], "gpt-test");
        assert_eq!(out["messages"][0]["role"], "system");
        assert_eq!(out["messages"][0]["content"], "brief");
        assert_eq!(out["messages"][1]["content"], "hi");
        assert_eq!(
            out["messages"][2]["tool_calls"][0]["function"]["name"],
            "ls"
        );
        assert_eq!(out["messages"][3]["role"], "tool");
        assert_eq!(out["max_tokens"], 99);
        assert_eq!(out["tools"][0]["function"]["name"], "ls");
    }

    #[test]
    fn resp_maps_tool_call() {
        let body = json!({
            "model": "gpt-test",
            "choices": [{ "finish_reason": "tool_calls", "message": {
                "content": "checking",
                "tool_calls": [{ "id": "c1", "type": "function",
                    "function": { "name": "ls", "arguments": "{\"path\":\"/tmp\"}" } }]
            }}],
            "usage": { "prompt_tokens": 4, "completion_tokens": 6 }
        });
        let out = response(&body);
        let parts = out.pointer("/candidates/0/content/parts").unwrap();
        assert_eq!(parts[0]["text"], "checking");
        assert_eq!(parts[1]["functionCall"]["name"], "ls");
        assert_eq!(parts[1]["functionCall"]["args"]["path"], "/tmp");
        assert_eq!(
            out.pointer("/candidates/0/finishReason"),
            Some(&json!("STOP"))
        );
        assert_eq!(
            out.pointer("/usageMetadata/promptTokenCount"),
            Some(&json!(4))
        );
        assert_eq!(
            out.pointer("/usageMetadata/candidatesTokenCount"),
            Some(&json!(6))
        );
    }

    #[test]
    fn sse_text_flow() {
        let mut c = GeminiClientSseConverter::new();
        let out = c.feed(
            br#"data: {"choices":[{"delta":{"role":"assistant","content":"Hel"}}]}

"#,
        );
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\"text\":\"Hel\""), "{s}");
        assert!(s.contains("\"role\":\"model\""), "{s}");
        let out = c.feed(br#"data: {"choices":[{"delta":{"content":"lo"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":2}}

data: [DONE]

"#);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\"text\":\"lo\""), "{s}");
        assert!(s.contains("\"finishReason\":\"STOP\""), "{s}");
        assert!(s.contains("\"promptTokenCount\":1"), "{s}");
        assert!(!s.contains("[DONE]"), "不应透传 [DONE]: {s}");
        // 结束后再 finish 不重复补块
        assert!(c.finish().is_empty());
    }

    #[test]
    fn sse_tool_flow() {
        let mut c = GeminiClientSseConverter::new();
        let out = c.feed(br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"ls","arguments":"{\"pa"}}]}}]}

"#);
        assert!(
            out.is_empty(),
            "聚合中不输出: {:?}",
            String::from_utf8_lossy(&out)
        );
        let out = c.feed(br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\":\"/\"}"}}]}}]}

"#);
        assert!(out.is_empty());
        let out = c.feed(
            br#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}

data: [DONE]

"#,
        );
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\"functionCall\""), "{s}");
        assert!(s.contains("\"name\":\"ls\""), "{s}");
        assert!(s.contains("\"path\":\"/\""), "{s}");
        assert!(s.contains("\"finishReason\":\"STOP\""), "{s}");
    }
}
