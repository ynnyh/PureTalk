# PureTalk / 清语

## 发版（第一硬性要求）

**发版前必须阅读 `RELEASE.md`。** 不读不发。

- 版本号四处一致：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`CHANGELOG.md`
- Tag 格式：`vX.Y.Z`
- CHANGELOG 必须填写，不可为空

## 项目概述

清语 — Windows 纯粹语音输入工具。Tauri 2 + Vue 3。

- 无主窗口，仅系统托盘
- 全局热键触发录音，转写文字注入当前输入框
- 本地引擎：sherpa-onnx + SenseVoice
- 云端引擎：火山引擎流式 ASR（可选）

## 关键路径

- 配置：`~/.puretalk/config.json`
- 语音资产：`~/.puretalk/voice/`
- 设置窗口：`src/windows/settings/`
- 提示条窗口：`src/windows/indicator/`
- 语音模块：`src-tauri/src/voice/`

## 构建

```bash
npm install
npm run tauri dev      # 开发
npm run tauri build    # 构建安装包
```
