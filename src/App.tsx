import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import type { AppType, Provider, ProxyStatus } from "./types";

const APP_TABS: AppType[] = ["claude", "codex", "gemini", "opencode", "openclaw"];

function App() {
  const [app, setApp] = useState<AppType>("claude");
  const [providers, setProviders] = useState<Provider[]>([]);
  const [proxy, setProxy] = useState<ProxyStatus | null>(null);
  const [keychainOk, setKeychainOk] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  // 添加供应商表单
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");

  async function refresh(target: AppType) {
    try {
      const list = await invoke<Provider[]>("list_providers", { appType: target });
      setProviders(list);
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  }

  async function refreshMeta() {
    try {
      setProxy(await invoke<ProxyStatus>("proxy_status"));
      await invoke("keychain_health");
      setKeychainOk(true);
    } catch (e) {
      setKeychainOk(false);
      setErr(String(e));
    }
  }

  useEffect(() => {
    refresh(app);
  }, [app]);

  useEffect(() => {
    refreshMeta();
  }, []);

  async function switchTo(id: string) {
    setBusy(true);
    try {
      await invoke("switch_provider", { id, appType: app });
      await refresh(app);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function create() {
    if (!name || !baseUrl) return;
    setBusy(true);
    try {
      await invoke("create_provider", {
        input: {
          app_type: app,
          name,
          base_url: baseUrl,
          models: [],
          api_key: apiKey || undefined,
        },
      });
      setName("");
      setBaseUrl("");
      setApiKey("");
      await refresh(app);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    setBusy(true);
    try {
      await invoke("delete_provider", { id });
      await refresh(app);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app">
      <header className="app__topbar">
        <div className="brand">
          <span className="brand__mark">◈</span>
          <span className="brand__name">Conduit</span>
          <span className="brand__tag">本地代理 · 凭证加密</span>
        </div>
        <div className="status">
          <span className={`dot ${proxy?.running ? "dot--ok" : "dot--bad"}`} />
          代理 {proxy?.running ? `运行中 · ${proxy.addr}` : "未运行"}
          <span className={`dot ${keychainOk ? "dot--ok" : keychainOk === false ? "dot--bad" : ""}`} />
          Keychain {keychainOk ? "可用" : keychainOk === false ? "不可用" : "检测中"}
        </div>
      </header>

      <nav className="tabs">
        {APP_TABS.map((t) => (
          <button
            key={t}
            className={`tab ${t === app ? "tab--active" : ""}`}
            onClick={() => setApp(t)}
          >
            {t}
          </button>
        ))}
      </nav>

      <main className="main">
        <section className="panel panel--providers">
          <h2>供应商 · {app}</h2>
          {providers.length === 0 && <p className="muted">暂无供应商,在右侧添加。</p>}
          <ul className="prov-list">
            {providers.map((p) => (
              <li key={p.id} className={`prov ${p.is_current ? "prov--current" : ""}`}>
                <div className="prov__main">
                  <div className="prov__name">
                    {p.name}
                    {p.is_current && <span className="badge">当前</span>}
                  </div>
                  <div className="prov__url">{p.base_url}</div>
                  <div className="prov__meta">
                    <span className={`pill ${p.has_key ? "pill--ok" : "pill--warn"}`}>
                      {p.has_key ? "Key 已配置" : "无 Key"}
                    </span>
                  </div>
                </div>
                <div className="prov__actions">
                  {!p.is_current && (
                    <button disabled={busy} onClick={() => switchTo(p.id)}>
                      切换
                    </button>
                  )}
                  <button className="ghost" disabled={busy} onClick={() => remove(p.id)}>
                    删除
                  </button>
                </div>
              </li>
            ))}
          </ul>
          {err && <p className="error">{err}</p>}
        </section>

        <section className="panel panel--add">
          <h2>添加供应商</h2>
          <label>
            名称
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="例如 CoderPlan" />
          </label>
          <label>
            Base URL
            <input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.example.com"
            />
          </label>
          <label>
            API Key(存入系统 keychain,不落盘)
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
            />
          </label>
          <button className="primary" disabled={busy || !name || !baseUrl} onClick={create}>
            {busy ? "处理中…" : "添加"}
          </button>
          <p className="hint">
            切换供应商只改本地状态,下一个请求立即生效 —— 所有 CLI 免重启。
          </p>
        </section>
      </main>
    </div>
  );
}

export default App;
