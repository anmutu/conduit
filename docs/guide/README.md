# Conduit 产品说明

> AI CLI 的本地供应中心 —— 一个应用管住所有 AI 编程工具的供应商、密钥与用量。

---

## 一、Conduit 解决什么问题

如果你同时用多个 AI 编程 CLI(Claude Code、Codex、Gemini CLI……),大概率撞过这些墙:

| 痛点 | 具体场景 |
|---|---|
| **配置割裂** | 每个 CLI 一套配置格式(JSON / TOML / .env),换一家供应商要手动改文件、查文档 |
| **切换要重启** | Codex 切完供应商必须重开终端会话,正在跑的活儿被打断 |
| **Key 明文裸奔** | API Key 明文躺在 `settings.json` / `config.toml` 里,误传仓库、被同步工具带上云,都是真实事故 |
| **多供应商管理乱** | 主力 + 备用 + 官方订阅 + 各家国产模型(GLM/Kimi),来回注释改配置 |
| **花费看不清** | 每家供应商各用多少 token,没有统一账本 |

**Conduit 的答案:本地代理优先。** 所有 CLI 的请求先经过 Conduit 的本地代理(`127.0.0.1:9527`),再转发到当前供应商——切换供应商变成"改一个指针",以上问题一次解决。

```
Claude Code ┐
Codex       │──► Conduit 本地代理 ──► 当前供应商(GLM / Kimi / 官方 / 任意中转)
Gemini CLI  │    (Key 从钥匙串注入)     ↑ 一键切换,免重启
OpenCode    │
OpenClaw    ┘
```

## 二、核心特点

### 1. 切换即生效,零重启 ⚡
代理按「当前供应商」转发。点一下卡片(或托盘)切换,下一个请求立即走新供应商——Codex、Gemini 和 Claude Code 一视同仁,不再有"重启终端"这回事。

### 2. Key 永不落盘明文 🔐
- API Key 存**系统钥匙串**(macOS Keychain / Windows DPAPI / Linux Secret Service)
- 配置数据库 **SQLCipher 整库加密**,主密钥也在钥匙串——配置文件泄露也读不出任何凭证
- 接管前的原配置自动备份进加密库,一键还原

### 3. 转发即计量 📊
代理转发的同时原生统计 token 用量(兼容 Anthropic / OpenAI 两种 usage 格式),每个供应商的请求数与 in/out tokens 直接显示在卡片上,零额外请求、零延迟。

### 4. 一键接管,随时还原 🛰️
「接管」把各 CLI 的配置指向本地代理(改前自动备份);「还原」恢复原始配置。状态面板还能发现"配置被外部工具修改"并及时修复。

### 5. 装上即用的细节 🧰
- **首启导入**:自动发现你现有的第三方配置,一键建好供应商
- **托盘快速切换**:菜单栏右键即切,不用开窗口;关闭窗口驻留托盘,代理不断流
- **中英双语**:默认跟随系统,界面与托盘一起切换
- **深浅双主题 / 快捷键**(`⌘N` 新建、`⌘1..5` 切分组)/ **开机自启**
- **轻量**:安装包约 9MB(Tauri 2,非 Electron)

## 三、怎么使用(5 分钟上手)

### 安装并启动
下载安装包(macOS dmg / Windows / Linux,见官网),首次启动时 macOS 弹出钥匙串授权请选「**始终允许**」——那是 Conduit 在保存加密主密钥。

### 三步配置(以 Codex 为例,其他 CLI 同理)
1. **添加供应商**:顶部切到 Codex 分组 → 右上 `+`(或 `⌘N`)→ 填名称、接口地址、API Key → 添加(Key 直接进钥匙串)
2. **设为当前**:卡片上点「切换」
3. **接管**:点顶栏绿色「代理运行中」→ 对应 CLI 行点「接管」

之后正常使用 `codex` / `claude` / `gemini` 命令即可——流量已走 Conduit。

> 没有现成供应商?看看 [GLM 接入](./providers/glm.md) 和 [Kimi 接入](./providers/kimi.md),国产模型的 Coding 套餐都提供 Anthropic 兼容端点。

### 日常操作速查

| 想做什么 | 怎么做 |
|---|---|
| 切换供应商 | 卡片「切换」/ 托盘右键(免重启) |
| 添加 / 编辑 / 删除供应商 | `⌘N` / 卡片铅笔 / 卡片垃圾桶 |
| 复制接口地址 | 点卡片上的地址 |
| 回官方登录 | 接管面板「还原」 |
| 看用量 | 卡片右侧自动累计 |
| 换语言 / 主题 / 开机自启 | 设置(齿轮) |
| 让 CLI 恢复直连 | 接管面板对应行「还原」 |

## 四、数据与安全

| 数据 | 位置 | 保护 |
|---|---|---|
| API Key | 系统钥匙串 | 系统级加密 |
| 供应商配置 / 接管备份 / 用量 | 应用数据目录 `conduit.db` | SQLCipher 整库加密 |
| 各 CLI 原配置 | 接管前备份进加密库 | 还原时解密写回 |

所有数据完全留在本机,不上传任何服务器。

## 五、与 CC Switch 对比

| 能力 | Conduit | CC Switch |
|---|---|---|
| Codex / Gemini 切换免重启 | ✅ 默认支持 | 需重启 / 手动开代理 |
| API Key 存储 | 系统钥匙串 + 整库加密 | 明文数据库 |
| 本地代理 | 默认开启 | 可选开启 |
| 关闭窗口后服务存活 | ✅ 驻留托盘 | — |
| 用量统计 | 转发即计量(原生) | Lua 脚本二次请求 |
| 多语言 | 中 / 英 | 中 / 英 / 日 |

> 致敬 [cc-switch](https://github.com/farion1231/cc-switch)(MIT)——Conduit 的视觉体系参考了它。

## 六、常见问题

**Q:接管会破坏我的现有配置吗?**
不会。接管前原配置完整备份进加密库,「还原」随时恢复;Claude 的 `settings.json` 只动 `ANTHROPIC_BASE_URL` 一项,Codex 的 `config.toml` 保留全部注释。

**Q:Conduit 没开,我的 CLI 还能用吗?**
接管模式下 CLI 会指向本地代理,Conduit 未运行则请求失败——建议开启「开机自启」(设置里),或让 Conduit 驻留托盘。介意的话随时「还原」回直连。

**Q:支持哪些供应商?**
任何 Anthropic / OpenAI / Gemini 兼容的供应商:官方、GLM、Kimi、DeepSeek、各类中转站,或自建网关。

**Q:和 cc-switch 能同时用吗?**
不建议(都会改写同一批 CLI 配置文件,互相覆盖)。二选一即可;从 cc-switch 迁移:在 Conduit 里用「从现有 CLI 配置导入」一键带入。

## 七、文档索引

| 文档 | 内容 |
|---|---|
| [Claude Code 指南](./claude-code.md) | 接管/切换/官方订阅共存 |
| [Codex 指南](./codex.md) | 首个详细指南,含完整故障排查 |
| [Gemini CLI 指南](./gemini.md) | Gemini 格式供应商接入 |
| [GLM 接入](./providers/glm.md) | 智谱 Coding Plan 双端点接法 |
| [Kimi 接入](./providers/kimi.md) | Moonshot Anthropic 端点接法 |

---

MIT 开源 · 问题反馈:[GitHub Issues](https://github.com/anmutu/conduit)
