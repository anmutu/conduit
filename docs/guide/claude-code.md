# Claude Code 使用指南

> 在 Conduit 中配置并管理 Anthropic Claude Code 的供应商:添加 Key、一键接管、免重启切换。

## 它是怎么工作的

```
claude CLI ──► Conduit 本地代理(127.0.0.1:9527)──► 当前供应商(转发 + 注入 Key)
```

- **接管**会把 `~/.claude/settings.json` 中 `env.ANTHROPIC_BASE_URL` 改写为 `http://127.0.0.1:9527`(改写前自动备份,可随时还原),`settings.json` 里的其他配置原样保留
- 你的 **API Key 存在系统钥匙串**,不落盘、不入库;转发请求时才由代理注入
- **Claude Code 是五个 CLI 里唯一「完全零重启」的**:它自带配置热重载——接管、切换供应商,正在运行的会话立即生效,连重开都不用

## 前置条件

| 需要 | 说明 |
|---|---|
| Claude Code CLI | 已安装并可运行 `claude` 命令 |
| 一个 Anthropic 兼容供应商 | 供应商的 `base_url` + `API Key`(如 GLM Coding Plan、Kimi,见 `providers/` 目录) |
| Conduit | 已启动,顶栏绿色「代理运行中」 |

## 配置四步

### ① 切换到 Claude 分组(默认)

### ② 添加供应商(`⌘N`)

| 字段 | 填写说明 | 示例 |
|---|---|---|
| 名称 | 自定义 | `GLM Coding Plan` |
| 接口地址 | 供应商的 **Anthropic 兼容**地址(根地址,不带 `/v1`) | `https://open.bigmodel.cn/api/anthropic` |
| API Key | 供应商的 Key(入钥匙串) | `sk-...` |

常见供应商的地址见 [`providers/`](./providers/) 目录下的接入文档。

### ③ 设为当前 — 卡片 hover → 「切换」

### ④ 接管 — 顶栏「代理运行中」→ Claude 行 → 「接管」

完成。**正在跑的 claude 会话直接继续用即可**,下一个请求已经走 Conduit。

## 日常使用

- **切换供应商**:卡片「切换」或托盘右键(Claude → 选供应商),**正在运行的会话立即生效**
- **用量**:卡片自动累积「N 次请求 ↓↑ tokens」
- **编辑**:铅笔图标;Key 留空不变
- **官方订阅用户**:想临时回官方 OAuth 登录,接管面板点「还原」;想两边常驻,把官方也加成一个供应商(地址留空或填官方地址、Key 填官方的)再一键互切

## 取消接管

顶栏「代理运行中」→ Claude 行 → 「还原」。`settings.json` 的 `ANTHROPIC_BASE_URL` 恢复为接管前的原值(从加密备份还原);若接管前没有该配置则自动移除。

## 故障排查

| 现象 | 处理 |
|---|---|
| 502「未设置 claude 的当前供应商」 | Claude 分组里没有任何卡片被设为「当前」→ 主界面点一次「切换」 |
| 401 / 鉴权失败 | 供应商 Key 不对:编辑供应商重填;GLM/Kimi 注意用各自平台的 Key |
| 404 / model not found | 供应商不支持当前模型名;claude 的 `model` 设置改为该供应商支持的模型(如 glm-4.6、kimi-k2) |
| ⚠️「配置被外部修改」 | `claude` 或其他工具改写了 settings.json → 重新「接管」一次 |
| 请求走了旧供应商 | Claude Code 极旧版本无热重载 → 重开一次会话(仅此场景) |
| 钥匙串反复弹窗 | 钥匙串访问 → 搜 `conduit` → 「始终允许」 |

## 与 Codex / Gemini 指南的差异速览

| | Claude Code | Codex | Gemini CLI |
|---|---|---|---|
| 接管改写的文件 | `~/.claude/settings.json` | `~/.codex/config.toml` | `~/.gemini/.env` |
| 接管后需重开会话 | ❌ 完全不用 | ✅ 一次 | ❌ 不用(每请求重读) |
| 切换供应商需重启 | ❌ | ❌ | ❌ |

---

相关文档:[Codex](./codex.md) · [Gemini](./gemini.md) · [GLM 接入](./providers/glm.md) · [Kimi 接入](./providers/kimi.md)
