# Conduit 设计文档

> 面向个人开发者的本地 AI 代理 + 多 CLI 供应商管理器。竞品:CC Switch。

## 1. 一句话定位

CC Switch 是"配置切换器,代理可选";**Conduit 是"本地代理优先,切换是代理的自然结果"**。
装上即全 CLI 热切换、API Key 不落盘明文。

## 2. 差异化主轴

| 主轴 | CC Switch 现状(实证) | Conduit 做法 |
|---|---|---|
| 真热切换 | Codex 必须重启;代理热切换要手动开 takeover | **默认代理接管**,切换只改 DB `is_current`,下个请求即生效 |
| 凭证加密 | API Key 明文落 SQLite + 明文接管备份 | **OS keychain 存 Key + SQLCipher 整库加密**,主密钥也在 keychain |

推迟项(已论证):成本归因(M2)、团队(M3)、wrapper 注入(不做,代理路线更稳)。

## 3. 技术栈

- **桌面**:Tauri 2.8+(Rust 后端 + React 19/TS 前端)
- **代理**:axum 0.8 + reqwest 0.13(流式转发,支持 SSE)
- **DB**:rusqlite 0.40 + `bundled-sqlcipher-vendored-openssl` + r2d2 连接池 + WAL
- **凭证**:`keyring` 4(macOS Keychain / Win DPAPI / Linux Secret Service)
- **前端**:Vite 7(后续 M1 引入 Tailwind + shadcn)

## 4. 架构

```
前端 React ──IPC──▶ commands/ (薄)
                       │
                  services/ (provider · keychain)
                       │
                   db/ (pool + SQLCipher + WAL)
                       
  本地代理 axum :9527 ◄── CLI 流量(takeover 后 base_url 指向此)
    按 URL 分流 → 查 current provider → keychain 取 Key → 注入凭证 → 流式转发
```

**设计原则(对标竞品痛点):**
- 代理优先:takeover 默认开,切换不写 live 文件 → 免重启
- Key 与配置分离:`Provider` 结构不含 key 字段;DB 只存 `keychain_id` 引用
- 并发友好:r2d2 池 + WAL 替代竞品"单 Mutex 单连接";per-app 锁(待 M1)
- 接管可恢复:接管状态原子提交 + 启动校验(待 M1)

## 5. 凭证注入策略(按 AppType)

| AppType | 路径前缀 | 凭证形式 |
|---|---|---|
| Claude / OpenCode / OpenClaw | `/v1/messages` | `x-api-key` + `Authorization: Bearer` + `anthropic-version` |
| Codex | `/v1/chat/completions`、`/v1/responses` | `Authorization: Bearer` |
| Gemini | `/v1beta/` | URL query `?key=` |

## 6. DB Schema(M0)

```sql
providers(id, app_type, name, base_url, keychain_id, models JSON,
          is_current, is_healthy, sort_index, created_at, meta)
settings(key, value)
```

注意:`providers` 刻意不存 API Key 明文。

## 7. 文件地图(M0)

```
src-tauri/src/
  types.rs            # AppType, Provider(无 key 字段), ProviderInput
  state.rs            # AppState { db pool, http client }
  db/                 # mod.rs(池+SQLCipher) schema.rs provider_dao.rs
  services/           # keychain.rs provider.rs
  core/proxy/         # mod.rs server.rs(分流+转发)
  commands/           # provider.rs proxy.rs keychain.rs
  lib.rs              # 组装:tracing→keychain→db→spawn proxy→register
src/
  App.tsx types.ts App.css   # 三栏空壳,调通后端命令
```

## 8. 里程碑

- **M0(本次)**:脚手架 + 代理骨架 + 加密 DB + keychain + 前端空壳。目标:可运行、链路通。
- **M1(MVP)**:5 CLI takeover + provider CRUD + 免重启切换 + 托盘 + 故障转移 + 首启向导。
- **M2**:成本归因(项目/会话/模型)+ MCP/Skills + 云同步 + Deep Link。
- **M3**:团队配置共享 + 权限。

## 9. 已知 M0 遗留(待 M1)

- 代理内的 DB 调用是同步阻塞(M0 可接受);M1 用 `spawn_blocking` 或加 per-app 缓存
- 无 takeoff/接管(写各 CLI live 配置)—— M1 实现
- 无熔断/故障转移 —— M1 实现
- 前端无 Tailwind/shadcn —— M1 前引入
- 无托盘/自启动 —— M1 实现
