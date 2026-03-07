# Translation Recording Tool

一个基于 `Tauri + Vue 3 + Rust + SQLite` 的桌面选词翻译工具。

这个项目现在走的是“本地优先”路线：

- 单个英文单词：优先查本地词典 `ECDICT + WordNet`
- 音频、例句、近义词：用 `Free Dictionary` 做在线补全
- 句子翻译或本地未命中：回退到有道翻译 API
- 历史记录、收藏、配置：全部保存在本地 SQLite

## 功能

- 剪贴板翻译和全局快捷键触发
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
  - Free Dictionary API
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

### 4. 生产构建

```bash
npm run build
npm run tauri build
```

## 使用说明

### 单词查询

1. 复制一个英文单词
2. 按全局快捷键，或者点击“翻译剪贴板内容”
3. 弹窗会优先展示本地词典结果
4. 如果有音频、例句、近义词，会一并显示

### 句子翻译

1. 复制一个句子或本地词典未命中的内容
2. 应用会回退到在线翻译 API
3. 这时需要在设置页配置有道 `App Key / App Secret`

### 设置页

- API 配置：可选
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
npm run build              # 前端构建
npm run tauri dev          # 开发模式
cargo check                # Rust 编译检查
cargo test                 # Rust 测试
```

## 设计原则

- 本地优先，减少对外部 API 的硬依赖
- 把“词典查询”和“句子翻译”分开处理
- 用户历史数据和静态词典数据分库存放
- 不把大体积生成文件塞进 Git

## 已知限制

- 目前离线词典优先链路只覆盖“单个英文单词”
- 句子翻译仍依赖在线 API
- 音频目前主要来自 Free Dictionary，而不是离线真人发音资源

## 许可证

项目代码默认按仓库约定处理。  
第三方词典数据请分别遵守各自上游许可：

- ECDICT
- WordNet
- Free Dictionary API
