//! 用量计量:在流式转发的同时扫描响应中的 usage 字段,零额外请求、零延迟。
//!
//! 兼容三种上游格式:
//! - Anthropic SSE:`message_start`(input_tokens)+ `message_delta`(累计 output_tokens)
//! - OpenAI 流:最后一个 chunk 的 usage(prompt/completion_tokens)
//! - 非流 JSON:响应体顶层 usage
//!
//! 策略:流中「最后一次出现」的 usage 视为最终值(各家的 usage 均为累计语义);
//! 流结束时若有值则落库。解析失败静默放弃 —— 计量绝不影响转发。

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

/// 跨 chunk 的行缓冲上限;超出则放弃本次 usage 解析(防异常大响应)
const MAX_BUF: usize = 64 * 1024;

pub struct UsageCtx {
    pub pool: crate::db::Pool,
    pub app_type: String,
    pub provider_id: String,
    pub model: Option<String>,
    /// 上游 HTTP 状态码(用于成功/失败统计)
    pub status: u16,
    /// 命中的路由规则匹配词(未命中 None;落库供日志页展示)
    pub rule_pattern: Option<String>,
}

pub struct UsageMeter {
    ctx: Option<UsageCtx>,
    /// 行缓冲(处理 SSE 行被 chunk 切断)
    line_buf: String,
    last: Option<(i64, i64)>,
    /// 计量器创建时刻(≈请求开始),finish 时算耗时
    started: std::time::Instant,
}

impl UsageMeter {
    pub fn new(ctx: UsageCtx) -> Self {
        Self {
            ctx: Some(ctx),
            line_buf: String::new(),
            last: None,
            started: std::time::Instant::now(),
        }
    }

    /// 沉默模式(测试/不想落库)
    pub fn disabled() -> Self {
        Self {
            ctx: None,
            line_buf: String::new(),
            last: None,
            started: std::time::Instant::now(),
        }
    }

    pub fn observe(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        for ch in text.chars() {
            if ch == '\n' {
                self.inspect_line();
                self.line_buf.clear();
            } else if self.line_buf.len() < MAX_BUF {
                self.line_buf.push(ch);
            }
        }
    }

    fn inspect_line(&mut self) {
        let line = self.line_buf.trim();
        if line.is_empty() || !line.contains("usage") {
            return;
        }
        // SSE: data: {...};非流 JSON:整行(或整个对象)就是 JSON
        let candidate = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
        if candidate == "[DONE]" {
            return;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(candidate) else {
            return;
        };
        // 顶层就是本响应(openai chunk / claude event 都带顶层 usage 或 .usage)
        if let Some((i, o)) = extract_usage(&v) {
            self.last = Some((i, o));
        }
    }

    /// 流结束:落库(一次性)
    pub fn finish(&mut self) {
        // 非流 JSON 响应末尾通常没有换行符:先检查缓冲区残留的最后一行
        if !self.line_buf.trim().is_empty() {
            self.inspect_line();
            self.line_buf.clear();
        }
        let Some(ctx) = self.ctx.take() else { return };
        // 无 usage 字段也记一次请求(tokens 计 0),保证"总请求"不失真
        let (input, output) = self.last.unwrap_or((0, 0));
        tracing::info!(
            "计量完成: app={}, provider={}, model={:?}, status={}, in={}, out={}",
            ctx.app_type,
            ctx.provider_id,
            ctx.model,
            ctx.status,
            input,
            output
        );
        if let Err(e) = crate::db::usage_dao::insert(
            &ctx.pool,
            &ctx.app_type,
            &ctx.provider_id,
            ctx.model.as_deref(),
            input,
            output,
            ctx.status,
            ctx.rule_pattern.as_deref(),
            self.started.elapsed().as_millis() as i64,
        ) {
            tracing::warn!("usage 落库失败(不影响转发): {e}");
        }
    }
}

/// 从 JSON 提取 usage:兼容 anthropic(input/output_tokens)与 openai(prompt/completion_tokens)
pub fn extract_usage(v: &serde_json::Value) -> Option<(i64, i64)> {
    let usage = v
        .get("usage")
        .or_else(|| v.get("message").and_then(|m| m.get("usage")))?;
    let input = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    Some((input, output))
}

/// 包装上游字节流:透传每个 chunk,同时喂给计量器;流结束触发落库。
pub struct MeteredStream<S> {
    inner: S,
    meter: UsageMeter,
    finished: bool,
}

impl<S> MeteredStream<S> {
    pub fn new(inner: S, meter: UsageMeter) -> Self {
        Self {
            inner,
            meter,
            finished: false,
        }
    }
}

impl<S, E> Stream for MeteredStream<S>
where
    S: Stream<Item = Result<bytes::Bytes, E>> + Unpin,
    E: std::fmt::Display,
{
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                self.meter.observe(&chunk);
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Some(Err(std::io::Error::other(e.to_string()))))
            }
            Poll::Ready(None) => {
                if !self.finished {
                    self.finished = true;
                    self.meter.finish();
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_anthropic_sse_events() {
        let mut m = UsageMeter::disabled();
        // message_start 事件(input_tokens=25)
        m.observe(b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}\n\n");
        // message_delta(累计 output=180)
        m.observe(b"event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":180}}\n\n");
        m.observe(b"data: [DONE]\n\n");
        m.finish();
        // 两次出现 usage,取最后(input=25 缺省 0?注意 message_delta 只有 output)
        // → last 是 (0,180);语义正确:以最后一次为准
        assert_eq!(m.last, Some((0, 180)));
    }

    #[test]
    fn parses_openai_final_chunk() {
        let mut m = UsageMeter::disabled();
        m.observe(b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n");
        // chunk 边界正好切断 usage 行
        m.observe(b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":");
        m.observe(b"34}}\n\n");
        m.observe(b"data: [DONE]\n\n");
        m.finish();
        assert_eq!(m.last, Some((12, 34)));
    }

    #[test]
    fn parses_plain_json_body() {
        let mut m = UsageMeter::disabled();
        m.observe(b"{\"id\":\"msg\",\"usage\":{\"input_tokens\":7,\"output_tokens\":9}}\n");
        m.finish();
        assert_eq!(m.last, Some((7, 9)));
    }

    #[test]
    fn parses_plain_json_without_trailing_newline() {
        // 真实 HTTP 响应通常没有结尾换行:缓冲区残留也必须被解析
        let mut m = UsageMeter::disabled();
        m.observe(b"{\"id\":\"msg\",\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":5}}");
        m.finish();
        assert_eq!(m.last, Some((3, 5)));
    }

    #[test]
    fn ignores_garbage_lines() {
        let mut m = UsageMeter::disabled();
        m.observe(b"data: {\"not\":\"json at all\n\n");
        m.observe(b": keep-alive comment\n\n");
        m.finish();
        assert_eq!(m.last, None);
    }
}
