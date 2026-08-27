// 前端类型,镜像后端 src-tauri/src/types.rs。
// 注意:Provider 故意不含 api_key 字段 —— 后端永远不会把 Key 传给前端。

export type AppType =
  | "claude"
  | "codex"
  | "gemini"
  | "opencode"
  | "openclaw"
  | "qwen"
  | "iflow"
  | "crush"
  | "droid";

/** 上游协议:供应商按协议暴露不同端点 */
export type Protocol = "anthropic" | "openai" | "gemini";

/** 各分组使用的协议(与后端 AppType::protocol 一致) */
export const APP_PROTOCOL: Record<AppType, Protocol> = {
  claude: "anthropic",
  codex: "openai",
  gemini: "gemini",
  opencode: "anthropic",
  openclaw: "anthropic",
  qwen: "openai",
  iflow: "anthropic",
  crush: "anthropic",
  droid: "anthropic",
};

export interface Provider {
  id: string;
  app_type: AppType;
  name: string;
  base_url: string;
  /** 各协议端点 {anthropic|openai|gemini: base_url} */
  endpoints: Record<string, string>;
  keychain_id: string | null;
  models: string[];
  is_current: boolean;
  is_healthy: boolean;
  sort_index: number;
  created_at: number;
  has_key: boolean;
  last_test?: { ok: boolean; latency_ms: number; ts?: number } | null;
}

export interface ProviderInput {
  app_type: AppType;
  name: string;
  base_url: string;
  models: string[];
  api_key?: string;
}

export interface ProxyStatus {
  addr: string;
  running: boolean;
  supported_apps: string[];
}

export interface UsageSummary {
  requests: number;
  input_tokens: number;
  output_tokens: number;
}
