# Translation Recording Tool

一个基于 `Tauri + Vue 3 + Rust + SQLite` 的桌面选词翻译工具。

这个项目现在走的是“本地优先”路线：

- 单个英文单词：优先查本地词典 `ECDICT + WordNet`
- 音频、例句、近义词：用 `Free Dictionary` 做在线补全
- 句子翻译或本地未命中：回退到有道翻译 API
- 历史记录、收藏、配置：全部保存在本地 SQLite

## 功能

- 剪贴板翻译和全局快捷键触发
- 截图 OCR 翻译：选择屏幕/窗口截图后，发送到本机 OCR HTTP 服务识别文本并翻译
- 弹窗展示单词、音标、中文释义、英文释义
- 收藏、历史记录、详情页
- 关闭主窗口时可选择最小化到托盘
- 本地词典优先，不依赖有道也能查常见英文单词
- 支持例句、近义词展示

## 技术栈

- 前端：`Vue 3`、`TypeScript`、`Pinia`、`Vue Router`
- 桌面壳：`Tauri 2`
- 后端：`Rust`
- 数据库：`SQLite`
- 在线服务：
  - 有道翻译 API
  - Microsoft Translator Text API
  - Free Dictionary API
  - PaddleOCR / RapidOCR 本机 OCR HTTP 服务
- 离线词典：
  - `ECDICT`
  - `WordNet`

## 目录结构

```text
Translationrecordingtool/
├── src/                         # Vue 前端
│   ├── components/              # 公共组件
│   ├── lib/                     # 前端数据库初始化等工具
│   ├── router/                  # 路由
│   ├── stores/                  # Pinia 状态管理
│   └── views/                   # 主页面、弹窗、设置、详情
├── src-tauri/
│   ├── resources/               # Tauri 打包资源（dictionary.db 生成后放这里）
│   ├── src/
│   │   ├── clipboard.rs         # 剪贴板读取
│   │   ├── database.rs          # 翻译记录模型
│   │   ├── lib.rs               # Tauri 命令、快捷键、窗口和托盘逻辑
│   │   ├── local_dictionary.rs  # 本地词典查询和结果合并
│   │   └── translator.rs        # 在线翻译 / Free Dictionary 补全
│   └── tests/
│       └── local_dictionary_tests.rs
├── scripts/
│   └── build_dictionary.py      # 构建离线 dictionary.db
└── README.md
```

## 运行前提

- Node.js 18+
- Rust stable
- Tauri 2 所需的系统依赖
- Windows 环境下建议直接用 PowerShell

## 快速开始

### 1. 安装依赖

```bash
npm install
```

### 2. 生成离线词典

这一步会下载 `ECDICT` 和 `WordNet`，然后生成本地词典库：

```bash
npm run build:dictionary
```

生成结果：

- 输出文件：`src-tauri/resources/dictionary.db`
- 下载缓存：`.cache/dictionary/`

注意：`dictionary.db` 是生成文件，不纳入 Git。

### 3. 启动开发环境

```bash
npm run tauri dev
```

### 4. 启动本机 OCR 服务

开发模式下，截图 OCR 翻译会自动按设置页选择的引擎启动本机 OCR HTTP 服务，也可以手动启动用于排查问题：

```bash
npm run ocr:server
```

默认地址是 `http://127.0.0.1:8866/ocr`，在设置页填入同一个地址即可。服务健康检查地址是 `http://127.0.0.1:8866/health`。

### 5. 构建 OCR sidecar

生产打包前需要先生成 OCR sidecar。Windows x64 环境运行：

```bash
npm run ocr:sidecar:win
```

该命令会使用 `uv` 创建隔离 Python 环境，安装固定版本的 PaddleOCR / PaddlePaddle / RapidOCR / ONNX Runtime / PyInstaller，并生成：

```text
src-tauri/binaries/paddle-ocr-server-x86_64-pc-windows-msvc.exe
```

Tauri 打包时会把这个 exe 作为 sidecar 一起带上。生成文件体积较大，不纳入 Git。

当前 sidecar 包含 Python 运行时和固定版本的 PaddleOCR / PaddlePaddle / RapidOCR / ONNX Runtime 依赖，可以在打包后的桌面端自动启动 OCR HTTP 服务。应用启动时可按设置后台预热 OCR 服务，也可以在设置页手动预热、重启 OCR 或打开 OCR 日志。

如果要做完全离线和更快首次启动，把 PaddleOCR 模型文件放到下面的 profile 目录中：

```text
src-tauri/resources/ocr-models/lite/det
src-tauri/resources/ocr-models/lite/rec
src-tauri/resources/ocr-models/lite/cls
src-tauri/resources/ocr-models/standard/det
src-tauri/resources/ocr-models/standard/rec
src-tauri/resources/ocr-models/standard/cls
src-tauri/resources/ocr-models/accurate/det
src-tauri/resources/ocr-models/accurate/rec
src-tauri/resources/ocr-models/accurate/cls
```

设置页可选择 `lite` / `standard` / `accurate`。当所选 profile 的 `det` 和 `rec` 目录里有模型文件时，打包版会优先使用内置模型；否则继续沿用 PaddleOCR 默认缓存机制，首次识别时如果本机没有模型缓存，可能需要下载模型并导致第一次启动较慢。

Windows 下也可以用脚本准备模型目录：

```bash
npm run ocr:models:win -- -Profile standard
```

可选 profile 为 `lite`、`standard`、`accurate`。模型文件体积较大，不纳入 Git。

### 6. 生产构建

普通构建：

```bash
npm run build
npm run tauri build
```

带 OCR sidecar 的 Windows 构建：

```bash
npm run tauri:build:ocr
```

`tauri:build:ocr` 会先生成 `src-tauri/binaries/paddle-ocr-server-x86_64-pc-windows-msvc.exe`，再使用 `src-tauri/tauri.ocr-sidecar.conf.json` 把它作为 Tauri sidecar 打进安装包。这个 sidecar 是统一 OCR HTTP 服务，支持 PaddleOCR 和 RapidOCR 两种引擎。

## 使用说明

### 单词查询

1. 复制一个英文单词
2. 按全局快捷键，或者点击“翻译剪贴板内容”
3. 弹窗会优先展示本地词典结果
4. 如果有音频、例句、近义词，会一并显示

### 句子翻译

1. 复制一个句子或本地词典未命中的内容
2. 应用会回退到在线翻译 API
3. 这时需要在设置页配置有道 `App Key / App Secret`，或选择微软翻译并配置 `Microsoft Translator Key / Region`

### 截图 OCR 翻译

1. 在设置页选择 OCR 引擎：`PaddleOCR` 或 `RapidOCR`
2. 填写 `OCR HTTP 地址`，例如 `http://127.0.0.1:8866/ocr`
3. 可开启“启动时预热 OCR”，应用启动后会后台拉起 OCR 服务，首次截图等待更短
4. 点击“截图 OCR 翻译”，在系统选择器中选择要截图的屏幕或窗口
5. 应用会把 PNG base64 以 JSON `{ "image": "..." }` 发送到 OCR 地址
6. OCR 返回文本后，会使用当前选择的在线翻译服务翻译并保存到历史记录

PaddleOCR 支持 `lite / standard / accurate` 三档模型目录。模型文件默认不提交到 Git，可按需下载：

```powershell
npm run ocr:models:win
```

RapidOCR 使用自身 ONNX 模型，模型档位设置仅对 PaddleOCR 生效。

OCR 服务返回建议使用以下任一格式：

```json
{ "text": "recognized text" }
```

或 Paddle 风格的行结果：

```json
{ "result": [{ "recText": "first line" }, { "recText": "second line" }] }
```

### 设置页

- 翻译与 OCR 服务：有道、微软翻译、OCR 引擎、OCR HTTP 地址、预热、重启和日志入口
- 全局快捷键：支持修改
- 托盘行为：支持关闭时退出或最小化到托盘

## 数据存储

### 用户数据

运行时数据库位于 Tauri 应用数据目录，包含：

- 历史记录
- 收藏状态
- 用户配置同步后的翻译结果

### 离线词典库

`dictionary.db` 为只读词典库，包含三类表：

- `ecdict_entries`
- `wordnet_synonyms`
- `wordnet_glosses`
- `wordnet_examples`

## 常用命令

```bash
npm run build:dictionary   # 生成离线词典库
npm run ocr:server         # 启动本机 PaddleOCR HTTP 服务（默认兼容命令）
npm run ocr:server:paddle  # 启动 PaddleOCR HTTP 服务
npm run ocr:server:rapid   # 启动 RapidOCR HTTP 服务
npm run ocr:models:win     # 下载 PaddleOCR 模型到 src-tauri/resources/ocr-models
npm run ocr:sidecar:win    # 生成带 PaddleOCR/RapidOCR 依赖的 OCR sidecar
npm run build              # 前端构建
npm run tauri dev          # 开发模式
npm run verify:final       # 最终验证门禁（前端测试 + Rust 测试 + 编译检查 + 构建）
cargo check                # Rust 编译检查
cargo test                 # Rust 测试
```

## 桌面 Smoke 测试指南

运行 `npm run tauri dev` 后，按以下步骤验证真实桌面行为：

### 1. 启动验证
- 确认 Tauri dev 模式正常启动，无 panic 或编译错误
- 主窗口正常显示，托盘图标（如配置启用）出现在系统托盘

### 2. 设置保存
- 打开设置页，修改任意配置项（如快捷键、托盘行为）
- 关闭设置页后重新打开，确认修改已持久化

### 3. 快捷键翻译
- 在任意应用中复制一个英文单词
- 按下全局快捷键（默认配置下），确认 popup 窗口弹出
- popup 中展示单词查询结果（本地词典或在线翻译）

### 4. Popup 窗口行为
- **无标题栏**：popup 窗口不显示系统标题栏
- **可拖拽**：鼠标按住 popup 窗口区域可拖动
- **ESC 关闭**：按下 ESC 键 popup 关闭
- **关闭按钮**：点击关闭按钮后 popup 关闭
- 连续多次快捷键触发，确认 popup 正常响应且无崩溃

## 设计原则

- 本地优先，减少对外部 API 的硬依赖
- 把“词典查询”和“句子翻译”分开处理
- 用户历史数据和静态词典数据分库存放
- 不把大体积生成文件塞进 Git

## 已知限制

- 目前离线词典优先链路只覆盖“单个英文单词”
- 句子翻译仍依赖在线 API
- 音频目前主要来自 Free Dictionary，而不是离线真人发音资源

## 隐私和数据

- 历史记录、收藏和配置保存在本地 SQLite。
- 单词查询优先走本地词典；本地未命中或句子翻译会调用在线服务。
- 有道 `App Key / App Secret` 只应保存在本机配置中，不要提交到 Git。

## 常见问题

### 为什么第一次启动前要生成离线词典？

本地词典库体积较大，不适合直接放进仓库。`npm run build:dictionary` 会下载上游词典数据并生成 `src-tauri/resources/dictionary.db`。

### 不配置有道 API 能用吗？

可以查常见英文单词。本地词典未命中、句子翻译等在线能力需要配置有道 API。

### 构建失败先看哪里？

先确认 Node.js、Rust、Tauri 2 系统依赖都已安装，再分别运行 `npm run build`、`cargo check` 定位是前端还是 Rust 侧失败。

## 许可证

当前仓库尚未包含明确的 license 文件。正式分发或接受外部贡献前，建议补充明确的开源协议。
第三方词典数据请分别遵守各自上游许可：

- ECDICT
- WordNet
- Free Dictionary API
