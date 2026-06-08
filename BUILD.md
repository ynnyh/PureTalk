# 清语构建指南

## 环境要求

- Node.js 20+
- Rust stable（含 `cargo`）
- Windows: 首次 `tauri build` 会自动引导安装 NSIS（约 5MB）

## 开发

```bash
npm install
npm run tauri dev
```

启动后仅托盘图标可见，无窗口弹出。右键托盘 → 设置 进行配置。

## 构建安装包

```bash
npm run tauri build
```

产物位置：

| 平台 | 格式 | 路径 |
|---|---|---|
| Windows | NSIS 安装包 | `src-tauri/target/release/bundle/nsis/*.exe` |

## 构建产物体积参考

| 文件 | 大小 |
|---|---|
| 清语.exe（Rust 二进制） | ~8 MB |
| WebView 资源 | ~2 MB |
| **最终安装包（压缩后）** | **~10 MB** |

> 注：首次使用需下载语音资产（sherpa-onnx 引擎 ~22MB + SenseVoice 模型 ~228MB），存储在 `~/.puretalk/voice/`。

## CI/CD

Push tag `v*` 触发 GitHub Actions 自动构建并创建 Draft Release。

```bash
git tag v0.0.1
git push origin main --tags
```

## 语音资产

运行时下载，不打包进安装包：

| 文件 | 来源 | 大小 |
|---|---|---|
| sherpa-onnx-offline.exe | GitHub Releases (k2-fsa) | ~22MB (tar.bz2) |
| model.int8.onnx | HuggingFace (csukuangfj) | ~228MB |
| tokens.txt | HuggingFace (csukuangfj) | ~308KB |

存储路径：`~/.puretalk/voice/`
