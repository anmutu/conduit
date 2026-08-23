# Kimi(Moonshot)接入指南

> 把 Kimi / Moonshot 作为供应商接入 Conduit,供 Claude Code 调用。

## 基本信息

| 项 | 值 |
|---|---|
| **Anthropic 兼容端点**(接 Claude Code 用) | `https://api.moonshot.cn/anthropic` |
| API Key 获取 | [Moonshot 开放平台](https://platform.moonshot.cn) 控制台 |
| 官方文档 | [在 Claude Code 中使用 Kimi](https://platform.kimi.com/docs/guide/claude-code-kimi) |

> 端点以 Moonshot / Kimi 官方文档为准;Claude Code 版本更新可能影响兼容行为,遇到异常先看官方文档更新。

## 接入 Claude Code

1. Conduit 切到 **Claude** 分组 → `⌘N` 添加供应商:

| 字段 | 填 |
|---|---|
| 名称 | `Kimi` |
| 接口地址 | `https://api.moonshot.cn/anthropic` |
| API Key | Moonshot 控制台的 Key(`sk-...`) |

2. 卡片「切换」设为当前
3. 顶栏「代理运行中」→ Claude 行「接管」(已接管过可跳过)
4. 完成——`claude` 会话即时生效

详细流程见 [Claude Code 使用指南](../claude-code.md)。

## 接入 Codex(可选)

Moonshot 同样提供 OpenAI 兼容接口,需要时可在 Codex 分组用其 OpenAI 端点接入(地址见 Moonshot 开放平台文档),步骤同 [Codex 指南](../codex.md)。

## 为什么要通过 Conduit 用 Kimi

- GLM、Kimi、官方、其他中转**同时配好,一键互切**,`claude` 不用重启
- **Key 进钥匙串**,不明文落盘
- **用量统计**按供应商分开累计

## 常见问题

| 现象 | 处理 |
|---|---|
| 401 / 鉴权失败 | Key 无效或账户未充值;控制台核对 |
| 404 | 端点写错:必须是 `https://api.moonshot.cn/anthropic` |
| model not found | claude 的 `model` 设置改为 Kimi 支持的型号(如 `kimi-k2`,以官方文档为准) |
| 兼容性异常(流式断流等) | 参考官方文档与 [社区 issue](https://github.com/MoonshotAI/Kimi-K2.5/issues),必要时换稳定版本组合 |

---

相关:[GLM 接入](./glm.md) · [Claude Code 指南](../claude-code.md) · [Codex 指南](../codex.md)
