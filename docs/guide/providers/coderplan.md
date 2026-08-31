# CoderPlan 接入指南

> 把 [CoderPlan](https://coderplan.ai) 作为供应商接入 Conduit,供 Claude Code(及 Codex)调用。

## 基本信息

| 项 | 值 |
|---|---|
| **接口地址**(Anthropic 兼容) | `https://api.coderplan.ai` |
| API Key 获取 | [CoderPlan 控制台](https://coderplan.ai) API 密钥页 |
| 官方文档 | [Claude Code 接入教程](https://coderplan.ai/docs/setup/claude-code) · [API Key 配置](https://coderplan.ai/docs/setup/api-key) |

> 端点是根地址,**不要在末尾追加 `/v1`**,加了会请求失败。
> Key 是控制台生成的 `sk-` 开头密钥,不是浏览器登录 token。

## 接入 Claude Code(推荐,主用法)

1. Conduit 切到 **Claude** 分组 → `⌘N` 添加供应商:

| 字段 | 填 |
|---|---|
| 名称 | `CoderPlan` |
| 接口地址 | `https://api.coderplan.ai` |
| API Key | CoderPlan 控制台的 `sk-` Key |

2. 卡片「切换」设为当前
3. 顶栏「代理运行中」→ Claude 行「接管」(若已接管过其他供应商可跳过)
4. 完成——`claude` 会话即时生效,不用重启;建议先跑一个小任务确认可用

详细流程见 [Claude Code 使用指南](../claude-code.md)。

## 为什么要通过 Conduit 用 CoderPlan

- **多供应商并存的切换键**:CoderPlan、GLM、Kimi、官方同时配好,卡片/托盘一键互切,`claude` 不用重启
- **Key 进钥匙串**:不再以明文环境变量(`ANTHROPIC_AUTH_TOKEN`)形式留在 shell 配置里
- **用量统计**:CoderPlan 与其他供应商的消耗分开累计,一眼看清

## 常见问题

| 现象 | 处理 |
|---|---|
| 404 / 请求失败 | 端点写错了:必须是 `https://api.coderplan.ai`,不带 `/v1` |
| 401 | Key 无效,或用了浏览器登录 token;去控制台 API 密钥页核对 `sk-` Key |
| 额度疑问 | CoderPlan 按真实调用扣费,额度 / tokens / 费用明细见控制台 |
