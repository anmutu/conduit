# GLM(智谱)接入指南

> 把 GLM Coding Plan / 智谱 Open Platform 作为供应商接入 Conduit,供 Claude Code(及 Codex)调用。

## 基本信息

| 项 | 值 |
|---|---|
| **Anthropic 兼容端点**(接 Claude Code 用) | `https://open.bigmodel.cn/api/anthropic` |
| **OpenAI 兼容端点**(接 Codex 用) | `https://open.bigmodel.cn/api/paas/v4` |
| API Key 获取 | [智谱开放平台](https://open.bigmodel.cn) 控制台 |
| 官方文档 | [Claude API 兼容说明](https://docs.bigmodel.cn/cn/guide/develop/claude/introduction) · [Coding Plan 快速开始](https://docs.bigmodel.cn/cn/coding-plan/quick-start) |

> 端点地址以智谱官方文档为准;注意 Anthropic 端点必须精确到 `/api/anthropic`,少了或换了路径会 404。

## 接入 Claude Code(推荐,主用法)

1. Conduit 切到 **Claude** 分组 → `⌘N` 添加供应商:

| 字段 | 填 |
|---|---|
| 名称 | `GLM Coding Plan` |
| 接口地址 | `https://open.bigmodel.cn/api/anthropic` |
| API Key | 智谱控制台的 Key |

2. 卡片「切换」设为当前
3. 顶栏「代理运行中」→ Claude 行「接管」(若已接管过其他供应商可跳过)
4. 完成——`claude` 会话即时生效,不用重启

详细流程见 [Claude Code 使用指南](../claude-code.md)。

## 接入 Codex(可选)

GLM 也有 OpenAI 兼容端点,可同时接入 Codex 分组:

| 字段 | 填 |
|---|---|
| 名称 | `GLM (OpenAI 兼容)` |
| 接口地址 | `https://open.bigmodel.cn/api/paas/v4` |
| API Key | 同一把智谱 Key |

> 注意:模型名需用该端点支持的型号;资源包计费在 Claude Code 端点与 OpenAI 端点的支持范围不同,以官方 FAQ 为准。

## 为什么要通过 Conduit 用 GLM

- **多供应商并存的切换键**:GLM、Kimi、官方、其他中转同时配好,卡片/托盘一键互切,`claude` 不用重启
- **Key 进钥匙串**:不再明文躺在 `settings.json` 里
- **用量统计**:GLM 与其他供应商的消耗分开累计,一眼看清

## 常见问题

| 现象 | 处理 |
|---|---|
| 404 | 端点写错了:Anthropic 端点必须是 `https://open.bigmodel.cn/api/anthropic`(精确匹配) |
| 401 | Key 无效或未充值/Coding Plan 未生效;控制台核对 |
| model not found | claude 的 `model` 设置改为 GLM 支持的型号(如 `glm-4.6`,以官方文档为准) |
| 想用资源包 | 见官方 [Coding Plan FAQ](https://docs.bigmodel.cn/cn/coding-plan/faq);Claude Code 内资源包支持范围以官方说明为准 |

---

相关:[Kimi 接入](./kimi.md) · [Claude Code 指南](../claude-code.md) · [Codex 指南](../codex.md)
