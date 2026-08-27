//! OpenAI Responses API → chat/completions 桥接。
//!
//! 触发场景:Codex 等 Responses 协议客户端要使用只提供 chat/completions 的供应商
//! (官方 OpenAI 原生支持 /v1/responses,不需要此桥接;按供应商 KV
//! `responses.bridge.{pid}` = "1" 显式开启)。
//! 转发前把 Responses 请求体转成 chat/completions,回程把 chat 响应(含 SSE 流)
//! 转回 Responses 形态,对 CLI 透明。

use serde_json::{json, Value};

/// Responses 请求体 → chat/completions 请求体。
pub fn request(body: &Value) -> Value {
    let mut out = json!({});
    if let Some(m) = body.get("model").and_then(|v| v.as_str()) {
        out["model"] = json!(m);
    }
    if let Some(n) = body.get("max_output_tokens").and_then(|v| v.as_i64()) {
        out["max_tokens"] = json!(n);
    }
    for k in ["temperature", "top_p"] {
        if let Some(v) = body.get(k) {
            if !v.is_null() {
                out[k] = v.clone();
            }
        }
    }
    if let Some(e) = body.pointer("/reasoning/effort").and_then(|v| v.as_str()) {
        out["reasoning_effort"] = json!(e);
    }
    if body.get("stream").and_then(|v| v.as_bool()) == Some(true) {
        out["stream"] = json!(true);
        out["stream_options"] = json!({ "include_usage": true });
    }

    let mut messages: Vec<Value> = Vec::new();
    if let Some(sys) = body.get("instructions").and_then(|v| v.as_str()) {
        if !sys.is_empty() {
            messages.push(json!({ "role": "system", "content": sys }));
        }
    }
    if let Some(items) = body.get("input").and_then(|v| v.as_array()) {
        for item in items {
            match item.get("type").and_then(|t| t.as_str()) {
                // 工具结果:message role=tool
                Some("function_call_output") => {
                    let call_id = item.get("call_id").and_then(|c| c.as_str()).unwrap_or("");
                    let out_val = item.get("output").cloned().unwrap_or(json!(""));
                    let text = match out_val {
                        Value::String(s) => s,
                        v => v.to_string(),
                    };
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": text,
                    }));
                }
                // 助手历史里的函数调用 → tool_calls
                Some("function_call") => {
                    messages.push(json!({
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": item.get("call_id").and_then(|c| c.as_str()).unwrap_or(""),
                            "type": "function",
                            "function": {
                                "name": item.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                                "arguments": item.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}"),
                            }
                        }],
                    }));
                }
                // 普通 message(role user/assistant/system)
                _ => {
                    let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                    let text = item_text(item);
                    if !text.is_empty() {
                        messages.push(json!({ "role": role, "content": text }));
                    }
                }
            }
        }
    }
    out["messages"] = json!(messages);

    // tools:Responses 的扁平 function → chat 的嵌套格式
    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        let fns: Vec<Value> = tools
            .iter()
            .filter(|t| t.get("type").and_then(|x| x.as_str()) == Some("function"))
            .filter_map(|t| {
                let name = t.get("name")?.as_str()?;
                Some(json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": t.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                        "parameters": t.get("parameters").cloned().unwrap_or(json!({"type":"object"})),
                    }
                }))
            })
            .collect();
        if !fns.is_empty() {
            out["tools"] = json!(fns);
            out["tool_choice"] = match body.get("tool_choice").and_then(|c| c.as_str()) {
                Some("required") => json!("required"),
                Some("none") => json!("none"),
                _ => json!("auto"),
            };
        }
    }
    out
}

/// message item 的 content:字符串或 [{type:"input_text"/"output_text", text}] → 纯文本
fn item_text(item: &Value) -> String {
    match item.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// chat/completions 响应 → Responses 响应(非流式)。
pub fn response(body: &Value) -> Value {
    let msg = body
        .pointer("/choices/0/message")
        .cloned()
        .unwrap_or(json!({}));
    let mut output: Vec<Value> = Vec::new();
    if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
        if !text.is_empty() {
            output.push(json!({
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{ "type": "output_text", "text": text }],
            }));
        }
    }
    if let Some(calls) = msg.get("tool_calls").and_then(|c| c.as_array()) {
        for c in calls {
            output.push(json!({
                "type": "function_call",
                "status": "completed",
                "call_id": c.get("id").and_then(|i| i.as_str()).unwrap_or(""),
                "name": c.pointer("/function/name").and_then(|n| n.as_str()).unwrap_or(""),
                "arguments": c.pointer("/function/arguments").and_then(|a| a.as_str()).unwrap_or("{}"),
            }));
        }
    }
    if output.is_empty() {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{ "type": "output_text", "text": "" }],
        }));
    }
    let in_tok = body
        .pointer("/usage/prompt_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let out_tok = body
        .pointer("/usage/completion_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or("resp_bridge"),
        "object": "response",
        "created_at": body.get("created").and_then(|c| c.as_i64()).unwrap_or(0),
        "status": "completed",
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "output": output,
        "usage": {
            "input_tokens": in_tok,
            "output_tokens": out_tok,
            "total_tokens": in_tok + out_tok,
        },
    })
}

/// chat SSE 增量 → Responses SSE 事件流转换器。
/// 输出 `event: X\ndata: {...}\n\n` 行;chat 的 [DONE] 不透传,
/// 由本转换器在 usage 帧后发 response.completed 收尾。
#[derive(Default)]
pub struct ResponsesBridgeSseConverter {
    line_buf: String,
    /// 已聚合的内容与工具调用,completed 帧要用
    text: String,
    tool_calls: Vec<Value>,
    usage: Value,
    /// message item 是否已开场(response.output_item.added)
    msg_opened: bool,
    /// 已收尾(completed 已发)
    finished: bool,
}

impl ResponsesBridgeSseConverter {
    pub fn new() -> Self {
        Self {
            line_buf: String::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            usage: json!({}),
            msg_opened: false,
            finished: false,
        }
    }

    fn event(&self, name: &str, data: &Value) -> String {
        format!("event: {name}\ndata: {data}\n\n")
    }

    fn open_message(&mut self, resp_id: &str, model: &str) -> String {
        if self.msg_opened {
            return String::new();
        }
        self.msg_opened = true;
        let item = json!({"type": "message", "role": "assistant", "status": "in_progress",
                          "content": [], "id": "msg_bridge"});
        let mut s = self.event(
            "response.output_item.added",
            &json!({"type": "response.output_item.added", "output_index": 0,
                    "item": item, "response_id": resp_id}),
        );
        s.push_str(&self.event(
            "response.content_part.added",
            &json!({"type": "response.content_part.added", "item_id": "msg_bridge",
                    "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": ""}}),
        ));
        let _ = model;
        s
    }

    /// 喂入 chat SSE 原始字节,返回可下发的 Responses 事件串。
    pub fn feed(&mut self, chunk: &[u8]) -> String {
        self.line_buf.push_str(&String::from_utf8_lossy(chunk));
        let mut out = String::new();
        while let Some(pos) = self.line_buf.find('\n') {
            let line: String = self.line_buf.drain(..=pos).collect();
            let line = line.trim_end_matches(['\n', '\r']);
            let payload = line.strip_prefix("data:").map(str::trim);
            let Some(payload) = payload else { continue };
            if payload == "[DONE]" {
                out.push_str(&self.finish());
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(payload) else {
                continue;
            };
            out.push_str(&self.handle_chat_event(&v));
        }
        out
    }

    fn handle_chat_event(&mut self, v: &Value) -> String {
        let mut out = String::new();
        // 文本增量
        if let Some(d) = v
            .pointer("/choices/0/delta/content")
            .and_then(|c| c.as_str())
        {
            if !d.is_empty() {
                if !self.msg_opened {
                    let id = v
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("resp_bridge");
                    let model = v.get("model").and_then(|m| m.as_str()).unwrap_or("");
                    out.push_str(&self.open_message(id, model));
                }
                self.text.push_str(d);
                out.push_str(&self.event(
                    "response.output_text.delta",
                    &json!({"type": "response.output_text.delta", "item_id": "msg_bridge",
                            "output_index": 0, "content_index": 0, "delta": d}),
                ));
            }
        }
        // 工具调用增量:按 index 聚合 arguments;name 出现时开场
        if let Some(calls) = v
            .pointer("/choices/0/delta/tool_calls")
            .and_then(|c| c.as_array())
        {
            for c in calls {
                let idx = c.get("index").and_then(|i| i.as_i64()).unwrap_or(0) as usize;
                while self.tool_calls.len() <= idx {
                    self.tool_calls.push(json!({
                        "id": "", "type": "function",
                        "function": {"name": "", "arguments": ""},
                    }));
                }
                let slot = &mut self.tool_calls[idx];
                if let Some(id) = c.get("id").and_then(|i| i.as_str()) {
                    if !id.is_empty() {
                        slot["id"] = json!(id);
                    }
                }
                if let Some(name) = c.pointer("/function/name").and_then(|n| n.as_str()) {
                    if !name.is_empty() {
                        slot["function"]["name"] = json!(name);
                    }
                }
                if let Some(args) = c.pointer("/function/arguments").and_then(|a| a.as_str()) {
                    let prev = slot["function"]["arguments"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    slot["function"]["arguments"] = json!(format!("{prev}{args}"));
                }
            }
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            self.usage = u.clone();
        }
        if let Some(fin) = v
            .pointer("/choices/0/finish_reason")
            .and_then(|f| f.as_str())
        {
            if fin == "tool_calls" {
                out.push_str(&self.finish());
            }
        }
        out
    }

    /// 收尾:补齐 item/part done + function_call items + response.completed。
    pub fn finish(&mut self) -> String {
        if self.finished {
            return String::new();
        }
        self.finished = true;
        let mut out = String::new();
        if self.msg_opened {
            out.push_str(&self.event(
                "response.content_part.done",
                &json!({"type": "response.content_part.done", "item_id": "msg_bridge",
                        "output_index": 0, "content_index": 0,
                        "part": {"type": "output_text", "text": self.text}}),
            ));
            out.push_str(&self.event(
                "response.output_item.done",
                &json!({"type": "response.output_item.done", "output_index": 0,
                        "item": {"type": "message", "role": "assistant", "status": "completed",
                                 "id": "msg_bridge",
                                 "content": [{"type": "output_text", "text": self.text}]}}),
            ));
        }
        for (i, c) in self.tool_calls.iter().enumerate() {
            out.push_str(&self.event(
                "response.output_item.done",
                &json!({"type": "response.output_item.done", "output_index": i + 1,
                        "item": {"type": "function_call", "status": "completed",
                                 "call_id": c["id"], "name": c["function"]["name"],
                                 "arguments": c["function"]["arguments"]}}),
            ));
        }
        let in_tok = self
            .usage
            .pointer("/prompt_tokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let out_tok = self
            .usage
            .pointer("/completion_tokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        let mut output = Vec::new();
        if self.msg_opened {
            output.push(
                json!({"type": "message", "role": "assistant", "status": "completed",
                               "id": "msg_bridge",
                               "content": [{"type": "output_text", "text": self.text}]}),
            );
        }
        for c in &self.tool_calls {
            output.push(json!({"type": "function_call", "status": "completed",
                               "call_id": c["id"], "name": c["function"]["name"],
                               "arguments": c["function"]["arguments"]}));
        }
        out.push_str(&self.event(
            "response.completed",
            &json!({"type": "response.completed",
                    "response": {"id": "resp_bridge", "object": "response",
                                 "status": "completed", "output": output,
                                 "usage": {"input_tokens": in_tok, "output_tokens": out_tok,
                                           "total_tokens": in_tok + out_tok}}}),
        ));
        out
    }
}

/// 流适配器:chat SSE 原始流 → Responses SSE 事件流。
pub struct ResponsesBridgeConvertStream<S> {
    inner: S,
    conv: ResponsesBridgeSseConverter,
    done: bool,
}

impl<S> ResponsesBridgeConvertStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            conv: ResponsesBridgeSseConverter::new(),
            done: false,
        }
    }
}

impl<S, E> futures::Stream for ResponsesBridgeConvertStream<S>
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
                // 上游没发 [DONE]:补齐 completed(已发过则空,结束流)
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
    fn request_maps_instructions_input_and_tools() {
        let body = json!({
            "model": "gpt-5", "instructions": "be brief", "stream": true,
            "max_output_tokens": 512,
            "reasoning": {"effort": "high"},
            "input": [
                {"role": "user", "content": "hi"},
                {"type": "function_call", "call_id": "c1", "name": "ls", "arguments": "{\"a\":1}"},
                {"type": "function_call_output", "call_id": "c1", "output": "ok"},
            ],
            "tools": [{"type": "function", "name": "ls", "description": "list",
                       "parameters": {"type": "object"}}],
        });
        let out = request(&body);
        assert_eq!(out["messages"][0]["role"], "system");
        assert_eq!(out["messages"][0]["content"], "be brief");
        assert_eq!(out["messages"][1]["content"], "hi");
        assert_eq!(
            out["messages"][2]["tool_calls"][0]["function"]["name"],
            "ls"
        );
        assert_eq!(out["messages"][3]["role"], "tool");
        assert_eq!(out["messages"][3]["tool_call_id"], "c1");
        assert_eq!(out["max_tokens"], 512);
        assert_eq!(out["reasoning_effort"], "high");
        assert_eq!(out["tools"][0]["function"]["name"], "ls");
        assert_eq!(out["stream"], true);
    }

    #[test]
    fn response_maps_text_and_tool_calls() {
        let chat = json!({
            "id": "chatcmpl-1", "model": "gpt-5", "created": 123,
            "choices": [{"finish_reason": "tool_calls", "message": {
                "content": "checking",
                "tool_calls": [{"id": "c1", "type": "function",
                    "function": {"name": "ls", "arguments": "{\"a\":1}"}}],
            }}],
            "usage": {"prompt_tokens": 4, "completion_tokens": 2},
        });
        let out = response(&chat);
        assert_eq!(out["object"], "response");
        assert_eq!(out["status"], "completed");
        assert_eq!(out["output"][0]["type"], "message");
        assert_eq!(out["output"][0]["content"][0]["text"], "checking");
        assert_eq!(out["output"][1]["type"], "function_call");
        assert_eq!(out["output"][1]["call_id"], "c1");
        assert_eq!(out["usage"]["input_tokens"], 4);
    }

    #[test]
    fn sse_converter_emits_delta_and_completed() {
        let mut c = ResponsesBridgeSseConverter::new();
        let out = c.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"he\"}}]}\n");
        assert!(out.contains("event: response.output_text.delta"), "{out}");
        assert!(out.contains("response.output_item.added"), "{out}");
        let out2 = c.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"llo\"}}]}\n");
        assert!(out2.contains("llo"), "{out2}");
        let fin = c.feed(b"data: {\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\ndata: [DONE]\n");
        assert!(fin.contains("response.output_item.done"), "{fin}");
        assert!(fin.contains("response.completed"), "{fin}");
        assert!(fin.contains("\"input_tokens\":3"), "{fin}");
        // [DONE] 不透传
        assert!(!fin.contains("[DONE]"), "{fin}");
    }

    #[test]
    fn sse_converter_aggregates_tool_calls() {
        let mut c = ResponsesBridgeSseConverter::new();
        let e1 = json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c9",
            "function":{"name":"ls","arguments":"{\"a\""}}]}}]})
        .to_string();
        let e2 = json!({"choices":[{"delta":{"tool_calls":[{"index":0,
            "function":{"arguments":":1}"}}]}}]})
        .to_string();
        c.feed(format!("data: {e1}\n").as_bytes());
        c.feed(format!("data: {e2}\n").as_bytes());
        let fin = c.feed(b"data: [DONE]\n");
        assert!(fin.contains("function_call"), "{fin}");
        assert!(fin.contains("arguments"), "{fin}");
        assert!(fin.contains(":1}"), "{fin}");
    }
}
