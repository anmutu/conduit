//! Anthropic → OpenAI 协议转换。
//!
//! 触发场景:Claude Code 等 Anthropic 协议客户端要使用只提供 OpenAI 兼容端点的供应商。
//! 代理在转发前把请求体转成 /chat/completions 格式,回程把 OpenAI 响应(含 SSE 流)
//! 转回 Anthropic 格式,对 CLI 完全透明。
//!
//! 覆盖:system / messages(text、tool_use、tool_result)/ tools / tool_choice /
//! max_tokens / temperature / top_p / stream;流式含 text_delta 与 tool_calls 增量。

use serde_json::{json, Value};

/// Anthropic 请求体 → OpenAI 请求体。模型名原样透传(由上游或路由规则决定)。
pub fn request(body: &Value) -> Value {
    let mut out = json!({});
    if let Some(m) = body.get("model").and_then(|v| v.as_str()) {
        out["model"] = json!(m);
    }
    if let Some(n) = body.get("max_tokens").and_then(|v| v.as_i64()) {
        out["max_tokens"] = json!(n);
    }
    for k in ["temperature", "top_p"] {
        if let Some(v) = body.get(k) {
            if !v.is_null() {
                out[k] = v.clone();
            }
        }
    }
    if body.get("stream").and_then(|v| v.as_bool()) == Some(true) {
        out["stream"] = json!(true);
        out["stream_options"] = json!({ "include_usage": true });
    }

    let mut messages: Vec<Value> = Vec::new();
    // system:字符串或 content blocks
    if let Some(sys) = body.get("system") {
        let text = sys_to_text(sys);
        if !text.is_empty() {
            messages.push(json!({ "role": "system", "content": text }));
        }
    }
    if let Some(msgs) = body.get("messages").and_then(|v| v.as_array()) {
        for m in msgs {
            convert_message(m, &mut messages);
        }
    }
    out["messages"] = json!(messages);

    // tools → OpenAI function 格式
    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        let fns: Vec<Value> = tools
            .iter()
            .filter_map(|t| {
                let name = t.get("name")?.as_str()?;
                Some(json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": t.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                        "parameters": t.get("input_schema").cloned().unwrap_or(json!({"type":"object"})),
                    }
                }))
            })
            .collect();
        if !fns.is_empty() {
            out["tools"] = json!(fns);
            out["tool_choice"] = match body
                .get("tool_choice")
                .and_then(|c| c.get("type"))
                .and_then(|t| t.as_str())
            {
                Some("any") => json!("required"),
                Some("tool") => {
                    let name = body
                        .pointer("/tool_choice/name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    json!({ "type": "function", "function": { "name": name } })
                }
                _ => json!("auto"),
            };
        }
    }
    out
}

/// system 字段:字符串 或 [{type:"text",text}] → 纯文本
fn sys_to_text(sys: &Value) -> String {
    match sys {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// 单条 Anthropic message → 一或多条 OpenAI message(assistant 的 tool_use、
/// user 的 tool_result 需要拆成独立的 tool_calls / tool 角色)。
fn convert_message(m: &Value, out: &mut Vec<Value>) {
    let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
    let blocks = match m.get("content") {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::String(s)) => vec![json!({ "type": "text", "text": s })],
        _ => return,
    };

    if role == "assistant" {
        // 文本与 tool_calls 合入一条 assistant 消息
        let text: String = blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("");
        let tool_calls: Vec<Value> = blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
            .filter_map(|b| {
                Some(json!({
                    "id": b.get("id")?.as_str()?,
                    "type": "function",
                    "function": {
                        "name": b.get("name")?.as_str()?,
                        "arguments": b.get("input").cloned().unwrap_or(json!({})).to_string(),
                    }
                }))
            })
            .collect();
        let mut msg = json!({ "role": "assistant" });
        if !text.is_empty() {
            msg["content"] = json!(text);
        }
        if !tool_calls.is_empty() {
            msg["tool_calls"] = json!(tool_calls);
        }
        out.push(msg);
    } else {
        // user:tool_result 逐个拆成 tool 消息,其余文本合并为一条 user
        let mut text = String::new();
        for b in &blocks {
            if b.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                let call_id = b.get("tool_use_id").and_then(|i| i.as_str()).unwrap_or("");
                let content = match b.get("content") {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Array(a)) => a
                        .iter()
                        .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => String::new(),
                };
                out.push(json!({ "role": "tool", "tool_call_id": call_id, "content": content }));
            } else if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
        if !text.is_empty() {
            out.push(json!({ "role": "user", "content": text }));
        }
    }
}

/// OpenAI 非流式响应 → Anthropic message 响应。
pub fn response(body: &Value) -> Value {
    let choice = body.pointer("/choices/0").cloned().unwrap_or(json!({}));
    let msg = choice.get("message").cloned().unwrap_or(json!({}));
    let mut content: Vec<Value> = Vec::new();
    if let Some(t) = msg.get("content").and_then(|c| c.as_str()) {
        if !t.is_empty() {
            content.push(json!({ "type": "text", "text": t }));
        }
    }
    if let Some(calls) = msg.get("tool_calls").and_then(|c| c.as_array()) {
        for c in calls {
            let args = c
                .pointer("/function/arguments")
                .and_then(|a| a.as_str())
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or(json!({}));
            content.push(json!({
                "type": "tool_use",
                "id": c.get("id").and_then(|i| i.as_str()).unwrap_or("toolu_call_missing"),
                "name": c.pointer("/function/name").and_then(|n| n.as_str()).unwrap_or(""),
                "input": args,
            }));
        }
    }
    let stop_reason = match choice.get("finish_reason").and_then(|f| f.as_str()) {
        Some("tool_calls") | Some("function_call") => "tool_use",
        Some("length") => "max_tokens",
        _ => "end_turn",
    };
    json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or("msg_conv"),
        "type": "message",
        "role": "assistant",
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": body.pointer("/usage/prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
            "output_tokens": body.pointer("/usage/completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        }
    })
}

// ============ SSE 流:OpenAI chunks → Anthropic events ============

/// 有状态转换器:喂入 OpenAI SSE 的字节流,吐出 Anthropic SSE 的完整行(含空行)。
/// `feed` 返回的是可直接写入响应体的字节。
pub struct SseConverter {
    /// 跨 chunk 行缓冲(SSE 事件以 \n 分隔)
    line_buf: String,
    /// 已发出的 content_block 序号(下一个 block 的 index)
    block_index: i64,
    /// 文本块是否已 start(0 号块固定给文本,惰性开启)
    text_open: bool,
    /// tool_calls 增量聚合:openai index → (block index, 已发 start?, name 缓冲, 参数缓冲)
    tools: std::collections::HashMap<i64, ToolAgg>,
    sent_stop: bool,
}

struct ToolAgg {
    block_index: i64,
    started: bool,
    id: String,
    name_buf: String,
    args_buf: String,
}

impl Default for SseConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl SseConverter {
    pub fn new() -> Self {
        Self {
            line_buf: String::new(),
            // 0 号固定留给文本块(即使没有文本也保持 Anthropic 常见的空文本块结构)
            block_index: 1,
            text_open: false,
            tools: std::collections::HashMap::new(),
            sent_stop: false,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.sent_stop
    }

    /// 喂入上游 chunk,返回转换后的 Anthropic SSE 字节
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.line_buf.push_str(&String::from_utf8_lossy(chunk));
        // 先把完整行拆出来(避免跨行持有 &self.line_buf 的借用)
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
                self.finish(&mut out);
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            self.on_chunk(&v, &mut out);
        }
        out
    }

    fn on_chunk(&mut self, v: &Value, out: &mut Vec<u8>) {
        if !self.text_open {
            // 首个事件之前先发 message_start + 文本块 start
            emit(
                out,
                "message_start",
                json!({ "type": "message_start", "message": {
                    "id": v.get("id").and_then(|x| x.as_str()).unwrap_or("msg_conv"),
                    "type": "message", "role": "assistant",
                    "model": v.get("model").and_then(|x| x.as_str()).unwrap_or(""),
                    "content": [], "stop_reason": null, "stop_sequence": null,
                    "usage": { "input_tokens": 0, "output_tokens": 0 }
                }}),
            );
            emit(
                out,
                "content_block_start",
                json!({ "type": "content_block_start", "index": 0,
                        "content_block": { "type": "text", "text": "" } }),
            );
            self.text_open = true;
        }

        let delta = v.pointer("/choices/0/delta").cloned().unwrap_or(json!({}));
        if let Some(t) = delta.get("content").and_then(|c| c.as_str()) {
            if !t.is_empty() {
                emit(
                    out,
                    "content_block_delta",
                    json!({ "type": "content_block_delta", "index": 0,
                            "delta": { "type": "text_delta", "text": t } }),
                );
            }
        }
        if let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
            for c in calls {
                self.on_tool_delta(c, out);
            }
        }
        // finish_reason → 关块 + message_delta(stop_reason + usage)
        let finish = v
            .pointer("/choices/0/finish_reason")
            .and_then(|f| f.as_str())
            .map(|s| s.to_string());
        if let Some(f) = finish {
            if self.text_open {
                emit(
                    out,
                    "content_block_stop",
                    json!({ "type": "content_block_stop", "index": 0 }),
                );
                self.text_open = false;
            }
            self.close_tools(out);
            let stop = match f.as_str() {
                "tool_calls" | "function_call" => "tool_use",
                "length" => "max_tokens",
                _ => "end_turn",
            };
            let usage = v.get("usage").cloned().unwrap_or(json!({}));
            emit(
                out,
                "message_delta",
                json!({ "type": "message_delta",
                        "delta": { "stop_reason": stop, "stop_sequence": null },
                        "usage": { "output_tokens": usage.get("completion_tokens").and_then(|x| x.as_i64()).unwrap_or(0) } }),
            );
            emit(out, "message_stop", json!({ "type": "message_stop" }));
            self.sent_stop = true;
        }
    }

    fn on_tool_delta(&mut self, c: &Value, out: &mut Vec<u8>) {
        let idx = c.get("index").and_then(|i| i.as_i64()).unwrap_or(0);
        let agg = self.tools.entry(idx).or_insert_with(|| ToolAgg {
            block_index: 0,
            started: false,
            id: String::new(),
            name_buf: String::new(),
            args_buf: String::new(),
        });
        if let Some(id) = c.get("id").and_then(|i| i.as_str()) {
            if agg.id.is_empty() {
                agg.id = id.to_string();
            }
        }
        if let Some(n) = c.pointer("/function/name").and_then(|n| n.as_str()) {
            agg.name_buf.push_str(n);
        }
        if let Some(a) = c.pointer("/function/arguments").and_then(|a| a.as_str()) {
            agg.args_buf.push_str(a);
            // 参数开始到达即认为 name 完整,开块(惰性,兼容 name 分片到达)
            if !agg.started && !agg.name_buf.is_empty() {
                self.start_tool(idx, out);
            }
        }
    }

    fn start_tool(&mut self, idx: i64, out: &mut Vec<u8>) {
        if let Some(agg) = self.tools.get_mut(&idx) {
            if agg.started {
                return;
            }
            agg.started = true;
            agg.block_index = self.block_index;
            let block_index = agg.block_index;
            let id = agg.id.clone();
            let name = agg.name_buf.clone();
            emit(
                out,
                "content_block_start",
                json!({ "type": "content_block_start", "index": block_index,
                        "content_block": { "type": "tool_use", "id": id, "name": name, "input": {} } }),
            );
            self.block_index += 1;
        }
    }

    fn close_tools(&mut self, out: &mut Vec<u8>) {
        let indexes: Vec<i64> = self.tools.keys().copied().collect();
        for idx in indexes {
            self.start_tool(idx, out); // 未开启的(空参数)在此兜底开启
            if let Some(agg) = self.tools.get_mut(&idx) {
                if agg.started && !agg.args_buf.is_empty() {
                    let bi = agg.block_index;
                    let args = std::mem::take(&mut agg.args_buf);
                    emit(
                        out,
                        "content_block_delta",
                        json!({ "type": "content_block_delta", "index": bi,
                                "delta": { "type": "input_json_delta", "partial_json": args } }),
                    );
                }
            }
        }
        let blocks: Vec<(i64, i64)> = self
            .tools
            .iter()
            .map(|(k, a)| (*k, a.block_index))
            .collect();
        for (_, bi) in blocks {
            emit(
                out,
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": bi }),
            );
        }
    }

    /// 兜底:上游没发 [DONE] / finish_reason 就断流时,补齐终止事件
    fn finish(&mut self, out: &mut Vec<u8>) {
        if self.sent_stop {
            return;
        }
        if !self.text_open && self.tools.is_empty() {
            return; // 一个事件都没发过,无从补齐
        }
        if self.text_open {
            emit(
                out,
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": 0 }),
            );
            self.text_open = false;
        }
        self.close_tools(out);
        emit(
            out,
            "message_delta",
            json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn", "stop_sequence": null } }),
        );
        emit(out, "message_stop", json!({ "type": "message_stop" }));
        self.sent_stop = true;
    }
}

fn emit(out: &mut Vec<u8>, event: &str, data: Value) {
    out.extend_from_slice(format!("event: {event}\ndata: {data}\n\n").as_bytes());
}

/// 流式转换器:上游 OpenAI SSE 流 → Anthropic SSE 流。
/// 上游断流而未发 [DONE] 时,结束时自动补齐终止事件。
pub struct ConvertStream<S> {
    inner: S,
    conv: SseConverter,
    done: bool,
}

impl<S> ConvertStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            conv: SseConverter::new(),
            done: false,
        }
    }
}

impl<S, E> futures::Stream for ConvertStream<S>
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
                    // 该 chunk 未产出完整事件,继续拉上游
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
                // 上游没发 [DONE]:补齐终止事件(已发过则返回空,结束流)
                let mut out = Vec::new();
                self.conv.finish(&mut out);
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
            "model": "gpt-x", "max_tokens": 100, "stream": false,
            "system": "be brief",
            "messages": [
                { "role": "user", "content": "hi" },
                { "role": "assistant", "content": [
                    { "type": "text", "text": "calling" },
                    { "type": "tool_use", "id": "t1", "name": "ls", "input": {"path": "/"} }
                ]},
                { "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": "t1", "content": "ok" }
                ]}
            ],
            "tools": [{ "name": "ls", "description": "list", "input_schema": {"type":"object"} }],
        });
        let out = request(&body);
        assert_eq!(out["messages"][0]["role"], "system");
        assert_eq!(out["messages"][1]["content"], "hi");
        assert_eq!(
            out["messages"][2]["tool_calls"][0]["function"]["name"],
            "ls"
        );
        assert_eq!(out["messages"][3]["role"], "tool");
        assert_eq!(out["tools"][0]["function"]["name"], "ls");
        assert_eq!(out["tool_choice"], "auto");
    }

    #[test]
    fn resp_maps_tool_call() {
        let body = json!({
            "id": "c1", "model": "gpt-x",
            "choices": [{ "finish_reason": "tool_calls", "message": {
                "content": null,
                "tool_calls": [{ "id": "call_1", "function": { "name": "ls", "arguments": "{\"path\":\"/tmp\"}" } }]
            }}],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
        });
        let out = response(&body);
        assert_eq!(out["stop_reason"], "tool_use");
        assert_eq!(out["content"][0]["type"], "tool_use");
        assert_eq!(out["content"][0]["input"]["path"], "/tmp");
        assert_eq!(out["usage"]["input_tokens"], 10);
    }

    #[test]
    fn sse_text_flow() {
        let mut c = SseConverter::new();
        let out = c.feed(b"data: {\"id\":\"1\",\"model\":\"m\",\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n");
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("event: message_start"));
        assert!(s.contains("text_delta"));
        assert!(!c.is_finished());
        let out = c.feed(
            b"data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}],\"usage\":{\"completion_tokens\":2}}\n\ndata: [DONE]\n\n",
        );
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\"stop_reason\":\"end_turn\""));
        assert!(s.contains("event: message_stop"));
        assert!(c.is_finished());
    }

    #[test]
    fn sse_tool_flow() {
        let mut c = SseConverter::new();
        let out = c.feed(b"data: {\"id\":\"1\",\"model\":\"m\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"ls\",\"arguments\":\"{\\\"pa\"}}]}}]}\n\n");
        let s = String::from_utf8(out).unwrap();
        assert!(
            s.contains("content_block_start"),
            "tool block should open: {s}"
        );
        let out = c.feed(br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\":\"/"}}]},"finish_reason":"tool_calls"}]}

data: [DONE]

"#);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("input_json_delta"));
        assert!(s.contains("\"stop_reason\":\"tool_use\""));
    }
}
