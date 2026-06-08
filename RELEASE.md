# 清语发版准则

**每次发版必读此文件。这是第一硬性要求。**

---

## 版本号规范

遵循语义化版本（SemVer）：`MAJOR.MINOR.PATCH`

| 递增 | 何时 | 示例 |
|---|---|---|
| **MAJOR** | 不兼容的架构变更、数据格式迁移 | `0.x.x → 1.0.0` |
| **MINOR** | 新增功能、新引擎、新平台支持 | `0.1.0 → 0.2.0` |
| **PATCH** | Bug 修复、文案调整、依赖更新 | `0.1.0 → 0.1.1` |

**版本号必须四处一致：**
- `package.json` → `"version"`
- `src-tauri/tauri.conf.json` → `"version"`
- `src-tauri/Cargo.toml` → `version`
- `CHANGELOG.md` → 顶部版本标题

**Tag 格式**：`v` + 版本号，如 `v0.0.1`、`v1.2.0`

---

## 发版前检查清单

逐项确认，全部通过才可发版：

### 1. 版本号一致性

```bash
# 四处版本号必须一致
grep '"version"' package.json
grep '"version"' src-tauri/tauri.conf.json
grep '^version' src-tauri/Cargo.toml
head -5 CHANGELOG.md
```

### 2. CHANGELOG

- [ ] 本次版本的 CHANGELOG 已填写（不可为空）
- [ ] 分类正确：新增 / 优化 / 修复 / 重构
- [ ] 每条描述简洁明确，一句话说清楚
- [ ] 无残留的 `Unreleased` 占位内容

### 3. 编译检查

```bash
cargo check                    # Rust 编译无错误
npm run build                  # 前端构建无错误
```

### 4. 功能验证

- [ ] `npm run tauri dev` 启动正常，无控制台报错
- [ ] 系统托盘图标显示正常，右键菜单可用
- [ ] 设置窗口打开/关闭正常，滚动条样式正确
- [ ] 本地引擎：录音 → 转写 → 文本注入 全流程通过
- [ ] 云端引擎：录音 → 转写 → 文本注入 全流程通过（如适用）
- [ ] 快捷键注册/切换正常
- [ ] indicator 提示条显示/隐藏正常，不抢焦点
- [ ] 资产下载（如有更新）代理/断点续传正常

### 5. 清理

- [ ] 无 `console.log` 调试残留
- [ ] 无 `eprintln!` 调试残留（生产日志除外）
- [ ] 无临时文件/测试代码

---

## 发版步骤

### Step 1：更新版本号

同步修改以下文件中的版本号：

```
package.json              → "version": "x.y.z"
src-tauri/tauri.conf.json → "version": "x.y.z"
src-tauri/Cargo.toml      → version = "x.y.z"
```

### Step 2：更新 CHANGELOG

在 `CHANGELOG.md` 顶部添加新版本条目：

```markdown
## vX.Y.Z

### 新增
- **功能名称**：一句话描述

### 优化
- **优化项**：一句话描述

### 修复
- **修复项**：一句话描述
```

### Step 3：提交 & 打 Tag

```bash
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml CHANGELOG.md
git commit -m "release: vX.Y.Z"
git tag vX.Y.Z
git push origin main --tags
```

### Step 4：CI 自动构建

Push tag 后 GitHub Actions 自动触发：
- Windows: NSIS 安装包 (`清语 vX.Y.Z_x64-setup.exe`)
- 自动创建 Draft Release，附带安装包

### Step 5：发布 Release

1. 进入 GitHub → Releases → 找到 Draft
2. 核对安装包已附带
3. 从 CHANGELOG 复制本次更新内容作为 Release Notes
4. 确认无误后 Publish

---

## CHANGELOG 写作规范

### 分类

| 类型 | 说明 | 关键词 |
|---|---|---|
| **新增** | 全新功能 | 新增、支持、接入 |
| **优化** | 已有功能改进 | 优化、改进、提升、调整 |
| **修复** | Bug 修复 | 修复、解决、修正 |
| **重构** | 代码重构（无功能变更） | 重构、拆分、整理 |

### 格式

- 每条以 `**粗体关键词**` 开头，冒号后接一句话描述
- 同类条目按重要程度排序
- 不写技术实现细节，只写用户可感知的变化
- 不写"修复了若干 bug"这种废话

### 示例

```markdown
## v0.1.0

### 新增
- **语音输入**：按热键说话，转写文字自动注入当前输入框
- **本地语音引擎**：SenseVoice 离线识别，无需联网，首次需下载模型约 250MB
- **云端语音引擎**：火山引擎流式 ASR，可选，更快更准
- **自定义热键**：设置内修改快捷键，支持 Ctrl/Shift/Alt 组合
- **下载代理**：支持配置 HTTP 代理，断点续传，自动重试

### 优化
- **设置窗口**：固定头部标题栏，内容区域独立滚动
- **下载进度**：实时显示速度、大小、百分比

### 修复
- **焦点抢占**：录音提示条不再抢夺输入框焦点
- **窗口关闭**：关闭设置窗口不再退出整个应用
```

---

## 热修复（Hotfix）流程

生产版本发现严重 Bug 时：

1. 从 `main` 切出 `hotfix/描述` 分支
2. 修复 + 更新 PATCH 版本号 + 更新 CHANGELOG
3. 合回 `main`，打 Tag，Push
4. 删除 hotfix 分支

---

## 版本生命周期

```
开发中 → CHANGELOG "Unreleased" 区域记录
准备发版 → 版本号确认 + 检查清单 + 打 Tag
已发布 → GitHub Release + CHANGELOG 归档
```

---

## 注意事项

- **不要跳版本号**：`0.0.1 → 0.0.3` 不允许，除非中间版本撤回
- **不要改已发布版本的 Tag**：有问题发新版本修复
- **不要在 main 上直接发版**：确保 CI 绿灯后再打 Tag
- **每次发版都必须更新 CHANGELOG**：哪怕只修了一个 typo
