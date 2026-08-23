<div align="center">

# Conduit

### AI CLI 的本地供应中心

[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg)](#)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange.svg)](https://tauri.app)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

一个应用管理 **Claude Code · Codex · Gemini CLI · OpenCode · OpenClaw** 的全部供应商。
切换即生效、免重启;API Key 加密存储、永不落盘明文。

[特性](#-核心特性) · [快速开始](#-快速开始) · [与 CC Switch 对比](#-与-cc-switch-对比)

|                  浅色                  |                  深色                  |
| :-----------------------------------: | :-----------------------------------: |
| ![浅色](docs/screenshots/app-light.png) | ![深色](docs/screenshots/app-dark.png) |

</div>

---

## ✨ 核心特性

- **⚡ 切换即生效,零重启** — 本地代理(`127.0.0.1:9527`)接管全部 CLI 流量,切换供应商只改一个指针,Codex / Gemini 与 Claude 一致免重启
- **🔐 Key 永不落盘明文** — API Key 存系统钥匙串(macOS Keychain / Windows DPAPI / Linux Secret Service),配置库整库 SQLCipher 加密
- **🛰️ 代理优先架构** — 代理是默认体验而非高级选项;关闭窗口驻留托盘,CLI 永不断流
- **🌗 浅色 / 深色 / 跟随系统** 三态主题,`⌘N` 新建、`⌘1..5` 快速切换应用

## 🚀 快速开始

```bash
pnpm install
pnpm tauri dev     # 开发(首次会请求钥匙串授权)
pnpm tauri build   # 构建
```

环境要求:Node 18+ / pnpm / Rust 1.85+ / Xcode CLT(macOS)。

## 🆚 与 CC Switch 对比

| 能力 | Conduit | CC Switch |
|---|---|---|
| Codex / Gemini 切换免重启 | ✅ 默认支持 | 需重启 / 手动开代理 |
| API Key 存储 | 系统钥匙串 + 整库加密 | 明文数据库 |
| 本地代理 | 默认开启 | 可选开启 |
| 关闭窗口后服务存活 | ✅ 驻留托盘 | — |
| 多 CLI 支持 | 5 个 | 6 个 |

> 致敬 [farion1231/cc-switch](https://github.com/farion1231/cc-switch)(MIT)——本项目的视觉体系参考了它。

## 🗺️ 路线图

- **M1**:takeover 接管(自动改写各 CLI live 配置)+ 首启导入 + 故障转移/熔断
- **M2**:成本归因(项目/会话/模型)+ MCP/Skills 管理 + 云同步
- **M3**:团队配置共享

详见 [docs/design.md](docs/design.md)。

## License

MIT
