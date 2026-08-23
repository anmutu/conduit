# Gemini CLI 使用指南

> 在 Conduit 中配置并管理 Google Gemini CLI 的供应商:添加 Key、一键接管、免重启切换。

## 它是怎么工作的

```
gemini CLI ──► Conduit 本地代理(127.0.0.1:9527)──► 当前供应商(转发 + 注入 Key)
```

- **接管**会在 `~/.gemini/.env` 写入一行 `GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:9527`(原文件整体备份,可随时还原;文件里其他内容如 `GEMINI_API_KEY` 原样保留)
- Gemini CLI **每次请求都重读 `.env`** —— 接管与切换供应商都即时生效,无需重启
- 你的 **API Key 存在系统钥匙串**;代理转发时以 URL 参数注入(与 Gemini API 的鉴权方式一致)

## 前置条件

| 需要 | 说明 |
|---|---|
| Gemini CLI | 已安装并可运行 `gemini` 命令 |
| 一个 **Gemini API 兼容**供应商 | 提供 Gemini 格式接口(`/v1beta/...`)的 `base_url` + Key。注意:普通 OpenAI 兼容中转不能直接用于 Gemini CLI,需确认其支持 Gemini 格式 |
| Conduit | 已启动,绿色「代理运行中」 |

## 配置四步

### ① 切换到 Gemini 分组

### ② 添加供应商(`⌘N`)

| 字段 | 填写说明 | 示例 |
|---|---|---|
| 名称 | 自定义 | `My Gemini Relay` |
| 接口地址 | 供应商的 Gemini 兼容根地址(不带 `/v1beta`) | `https://relay.example.com` |
| API Key | 供应商的 Key(入钥匙串) | `AIza...` |

### ③ 设为当前 — 卡片 hover → 「切换」

### ④ 接管 — 顶栏「代理运行中」→ Gemini 行 → 「接管」

若 `~/.gemini/.env` 不存在,接管时会自动创建。

## 日常使用

与其他分组一致:卡片/托盘切换(即时生效)、自动用量统计、铅笔编辑、垃圾桶删除。

## 取消接管

接管面板 → Gemini 行 → 「还原」。`.env` 恢复为接管前的原始内容。

## 故障排查

| 现象 | 处理 |
|---|---|
| 502「未设置 gemini 的当前供应商」 | Gemini 分组未设「当前」→ 主界面点「切换」 |
| 404 / model not found | 供应商不支持当前模型;`gemini` 的模型设置改为该供应商支持的型号 |
| 400 / 格式错误 | 供应商不是 Gemini 格式兼容(只是 OpenAI 兼容)——换支持 Gemini API 的供应商 |
| 官方 OAuth 登录失效/被影响 | 「还原」即回直连;Conduit 不改你的 Google 登录状态 |
| ⚠️「配置被外部修改」 | 重新「接管」一次 |

## 与其他指南的差异速览

| | Claude Code | Codex | Gemini CLI |
|---|---|---|---|
| 接管改写的文件 | `~/.claude/settings.json` | `~/.codex/config.toml` | `~/.gemini/.env` |
| 接管后需重开会话 | ❌ | ✅ 一次 | ❌(每请求重读) |
| Key 注入方式 | 请求头 | 请求头 | URL 参数(与 Gemini API 一致) |

---

相关文档:[Claude Code](./claude-code.md) · [Codex](./codex.md) · [GLM 接入](./providers/glm.md) · [Kimi 接入](./providers/kimi.md)
