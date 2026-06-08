# 清语 / PureTalk

> 纯粹的语音输入工具 — Windows 专属

[![Release](https://img.shields.io/github/v/release/ynnyh/PureTalk?label=下载)](https://github.com/ynnyh/PureTalk/releases)
[![Website](https://img.shields.io/badge/官网-puretalk.pages.dev-blue)](https://ynnyh.github.io/PureTalk)

**[English](#english)** | 中文

---

按下快捷键，说话，松开。文字出现在你正在输入的地方，仅此而已。

## 设计理念

- **纯粹录音** — 一键录音，一键停止，无多余交互
- **纯粹转写** — 不做大模型处理，不搞"智能优化"，只做语音转文字
- **纯粹输出** — 文字就是文字，复制即用，不自动发送
- **不打扰** — 不问不答，交互由用户掌控
- **零意外** — 每个功能都是有意为之，每个结果都可预期

## 数据安全

- 无需注册登录，下载即用
- 本地引擎完全离线，断网可用
- 语音在本机处理，不上传任何服务器
- 不收集使用数据，没有遥测埋点
- 代码完全开源，可审计每一行

## 使用方式

1. 从 [Releases](https://github.com/ynnyh/PureTalk/releases) 下载安装包，双击安装
2. 安装后自动弹出设置窗口，选择引擎并下载语音资产
3. 在任意输入框中按下 `Ctrl+Shift+Space` 开始录音
4. 再次按下停止录音，文字自动输入

后续可右键系统托盘图标 → 设置 进行修改。

## 技术栈

- **桌面框架**: Tauri 2 (Rust + WebView)
- **前端**: Vue 3 + Vite
- **本地 STT**: sherpa-onnx + SenseVoice (非自回归，CPU 亚秒级)
- **云端 STT**: 火山引擎流式 ASR (可选)

## 开发

```bash
npm install
npm run tauri dev
```

## 构建

```bash
npm run tauri build
```

详见 [BUILD.md](BUILD.md) | 发版规范见 [RELEASE.md](RELEASE.md)

## License

Apache 2.0

---

<a id="english"></a>

## English

**PureTalk** is a pure, minimal voice input tool for Windows.

Press the hotkey, speak, release. Text appears where you were typing. That's it.

### Principles

- **Pure recording** — one key to start, one key to stop, no extra UI
- **Pure transcription** — no LLM processing, no "smart optimization", speech-to-text only
- **Pure output** — text is text, copy and use, never auto-sent
- **Non-intrusive** — no popups, no prompts, user controls everything
- **Zero surprises** — every feature is intentional, every result is predictable

### Privacy & Security

- No registration, no login, download and use
- Local engine works fully offline
- Audio processed on-device, never uploaded
- No telemetry, no analytics, no data collection
- Fully open source, every line auditable

### Quick Start

1. Download the installer from [Releases](https://github.com/ynnyh/PureTalk/releases)
2. Launch — settings window opens automatically, download voice assets
3. Press `Ctrl+Shift+Space` in any text field to start recording
4. Press again to stop — text is typed automatically

### Tech Stack

- **Desktop**: Tauri 2 (Rust + WebView)
- **Frontend**: Vue 3 + Vite
- **Local STT**: sherpa-onnx + SenseVoice (non-autoregressive, sub-second on CPU)
- **Cloud STT**: Volcengine Streaming ASR (optional)

### License

Apache 2.0
