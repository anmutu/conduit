// 前端类型,镜像后端 src-tauri/src/types.rs。
// 注意:Provider 故意不含 api_key 字段 —— 后端永远不会把 Key 传给前端。

export type AppType = "claude" | "codex" | "gemini" | "opencode" | "openclaw";

export interface Provider {
  id: string;
  app_type: AppType;
  name: string;
  base_url: string;
  keychain_id: string | null;
  models: string[];
  is_current: boolean;
  is_healthy: boolean;
  sort_index: number;
  created_at: number;
  has_key: boolean;
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
