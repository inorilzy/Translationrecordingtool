# 仓库审计上下文

基线：`d72a76967f3f79d0e51eae4c049889d32f2e9a42`；开始时 `git status --short` 无输出。范围为源码/配置/开发文档，未读取实际 settings、历史数据库、运行日志或凭据。此文只建立边界、不变量与待核实假设，不判严重性、不建议修复。测试尚未由本任务执行。

## 规范与阅读方法

- 已读 `AGENTS.md:1-13` 和 `docs/agents/{domain,issue-tracker,triage-labels}.md`。工单为 `.scratch/<feature>/issues/<NN>-<slug>.md`，状态单独记录，采用五个 canonical triage 标签（`issue-tracker.md:5-11`，`triage-labels.md:5-11`）。
- 依 `docs/agents/domain.md:7-12` 检索域文档；遵守已有布局约定。架构说明参考 `docs/architecture-translation-workflow.md`，但以代码为当前事实。
- 已加载 audit-context-building，只使用“跟踪调用、区分保证与假设、保留未决问题”的方法；未调用其未注册子技能或创建子代理。
- 下文 `nothing found` 限指列出的已读调用路径未发现相应应用层检查，不代表已证明漏洞，也不宣称依赖内部没有限制。

## 调用边界地图

```text
Vue 页面 / Pinia ---- invoke ----------------------> lib.rs 命令适配器
全局快捷键 ---------- shortcut_handler ------------> AppTranslationWorkflow
系统 UIA / Clipboard / 截图 ------- 输入适配器 ------>      |
                                                        translation_flow
                                                        /            \
                                               DictionaryGateway   ProviderGateway
                                               local_dictionary    translator -> HTTPS
                                                        |
                                              TranslationRepository -> SQLite
图片输入 -> ocr facade -> spawn_blocking -> native_ocr -> ppocr-rs / ONNX DLL
Rust 分阶段事件 -> popup_window / main WebView -> Vue 渲染
SettingsRecord -> app_state 管理共享状态 -> settings.json
```

### 前端、Rust 与 IPC

- `src/App.vue:2-10,64-80`：仅 main 启动加载设置和历史；main 订阅 OCR 回填与导航事件。popup/screenshot-selection 共享入口但走独立 UI。
- `src/stores/translation.ts:76-153`：历史/收藏/详情与手动翻译使用 invoke；结果类型为前端手写 TS 接口，后端为 `database.rs:18-36` 的 serde record。TS 泛型本身不构成运行时验证。
- 图片路径：`translation.ts:155-195,202-270` -> `begin_image_translation` 或 `select_screenshot_area` -> `translate_image_overlay`；前端 operationId 与后端 requestId 双重过滤结果。Rust 入口见 `lib.rs:348-407,453-475`。
- `lib.rs:743-770` 是完整注册命令清单，涵盖翻译、图片、OCR 运维、历史、设置与日志。命令参数本身未接收/校验调用窗口身份；窗口级可达性须由安全审计核实 Tauri 生成权限语义，不能仅凭 capabilities 推断自定义命令已隔离。
- 设置前端 `stores/settings.ts:85-117` -> `get_settings/update_api_config`；Rust `lib.rs:320-323` 返回完整 SettingsRecord，包含 key 字段（`settings.rs:25-39`）。敏感字段经过 IPC 是设计事实，本任务未读取值。

### 工作流、并发与持久化

- 装配点 `lib.rs:586-597` 创建 canonical `Arc<RwLock<SettingsRecord>>`、workflow、popup/screenshot 两组 request 状态；`translation_workflow.rs:471-481` 注入五个 adapter。
- 文本每次请求读取配置快照 `translation_workflow.rs:166-178`；图片在 OCR 前一次性读取 translation+ocr 快照 `:290-328,439-445`，不依赖前端传 provider/credential。
- `translation_flow.rs:67-163`：trim/拒绝空白；单词先本地词典，命中后 Free Dictionary 补全；未命中时尝试 Free Dictionary 后 provider；句子直接 provider。词典读取错误会向上返回，不等同于 miss（`:87-88,230-239`）。
- 首个可用结果先写库再 report；补全更新不递增次数；完成结果内容相同则不再写（`translation_workflow.rs:192-255`）。repository 每次操作重新开连接（`:411-425`）。
- 取消是合作式 freshness 检查，出现在 stage 发布前与外部 await 后（`translation_flow.rs:101-102,185-216`；`translation_workflow.rs:290-309`）；并非中止已开始的 HTTP/OCR。手动文本/剪贴板命令明确传 `&|| false`（`lib.rs:325-345`）。
- SQLite 位于 app_data_dir/translations.db；每次 open 执行 schema 初始化及六个兼容列补齐（`database.rs:246-295`）。唯一键 `(source_text, source_lang, target_lang)`，不含 provider（`:114-140`）。参数化 UPSERT 依据该键合并、可递增 access_count、不覆盖收藏（`:316-366`）。
- 设置文件位于 app_config_dir；serde defaults 和 OCR normalization 后读取，写入为完整 JSON `fs::write`（`settings.rs:112-136`，`app_state.rs:67-88`）。更新先改内存后保存（`app_state.rs:96-161`）；快捷键命令有独立补偿路径（`lib.rs:136-299`）。
- 旧 app identifier 迁移只在目标文件不存在时复制 translations.db/dictionary.db（`app_state.rs:166-209`）。静态词典首次复制至 app data，已有文件直接复用（`local_dictionary.rs:34-59`）；词典缺失对查询为 Err（`:62-76`）。

### 系统输入、窗口与事件

- 快捷键注册 -> `handle_shortcut` -> UI Automation，失败/空值再临时 Ctrl+C -> workflow（`shortcut_handler.rs:22-103,252-319`）。恢复只保存成功读到的文本剪贴板（`:295-317`）；不要在审计中实际采集用户当前剪贴板。
- 截图快捷键与主窗口截图分别使用 popup 与 screenshot request 状态（`shortcut_handler.rs:105-157`，`lib.rs:348-475`）；两者共用 screenshot 全局选择 guard（`screenshot.rs:17-67,126-139`）。
- Windows 走原生 `screenshot/windows.rs`；非 Windows 走 WebView adapter（`screenshot.rs:130-147,235-239`）。公共区域校验有限值和最小 4px（`:183-195`）。非 Windows 通过完成/取消事件和 mpsc 等待，60s 选择超时、5s ready 超时（`screenshot/webview.rs:22-24,58-94`）。
- popup 创建隐藏本地 index.html，hash=/popup；ready 先标记 shared state，未 ready 的 stage 注册一次性回调，回调检查 requestId（`popup_window.rs:313-385`）。UI 事件订阅在 `PopupWindow.vue:60-108`，按钮可 invoke 收藏、打开主窗口（`:120-132`）。
- main OCR 回填有两种契约：图片 IPC 发 `{requestId,text}`（`lib.rs:362-365`），快捷键发纯字符串（`shortcut_handler.rs:183-187`）；store 无条件接受字符串，结构化 payload 过滤 active ID（`translation.ts:65-74`）。

### 权限、网络与日志

- `tauri.conf.json:21-65` 显式列 main/popup/screenshot-selection inline capability，CSP 为 null；另有 `capabilities/default.json:5-19` 和 `desktop.json:8-15`。需核实当前 Tauri 对显式 capability 列表与文件 capability 的实际加载关系，不能直接把权限简单相加。
- 外部 HTTP 共用连接 5s/请求 15s 超时 client（`http_client.rs:1-16`）。固定服务域名：Youdao（`translator.rs:107-123`）、Microsoft（`:161-197`）、Google（`:244-306`）、Free Dictionary（`:321-347`）。Google key 放 query，错误格式化 reqwest Error（`:255-275`）；后续安全审计检查错误串与日志脱敏语义。
- Free Dictionary 的非成功 HTTP 统一作 None（`translator.rs:338-340`）；provider 选择 microsoft/google，否则落到 Youdao（`translation_workflow.rs:368-400`）。只有 key 非空检查，未知 provider 不直接拒绝。
- 外部返回的 audio_url 可直接进入浏览器 `new Audio(...)`（`PopupWindow.vue:139-141`），是 Rust HTTP 以外的网络路径。
- 日志默认 info、按日轮转保留最多七文件（`logger.rs:8-25`）；翻译原文和划词原文写 info（`translation_flow.rs:80-85`，`shortcut_handler.rs:69`）。日志 IPC 接受 filename，join 后仅 starts_with，再 read_to_string（`logger.rs:56-74`）。路径规范化与目录成员验证未在该函数中找到，留给安全审计验证。

### OCR 与发布构建

- 产品 facade 始终 native（`ocr.rs:16-21`），recognition/init 在 spawn_blocking（`:41-65`）。warmup/restart 同样仅调用 ensure_running（`:68-74`）；status.running 来自模型目录存在，不是实际推理成功（`:84-107`）。
- native runtime 全局 Mutex，识别期间持有 runtime 和 engine 锁（`native_ocr.rs:36-72`）；相同 profile/path 初始化复用（`:125-146`）。OCR 将 blocks 排序后拼文本，转 axis-aligned bbox（`:80-121,349-359`）。
- base64/data URL -> bytes -> image::load_from_memory -> RGB（`native_ocr.rs:332-345`）；应用层图片字节/像素上限在此调用链 `nothing found`，依赖的默认解码限制仍需核实。
- 模型路径检查 det/rec ONNX + rec yml 文件存在（`native_ocr.rs:368-415`）。DLL 优先尊重 ORT_DYLIB_PATH，再找 resource/executable/current-dir/workspace，未找到交给系统 PATH/default（`:201-245`）。这些目录/环境变量可信度是运行环境假设。
- `package.json:9-19` 的 native build 先准备模型与 DLL，再 tauri build overlay config；`tauri.conf.json:68-74` 有 dictionary+models，overlay `tauri.ocr-native.conf.json:2-6` 仅列 models+DLL。合并后的 resources 是否仍含 dictionary 必须检查实际 resolved config/安装产物，不在此预判。
- 模型脚本从 HuggingFace `resolve/main` 下载，已有 inference.onnx 即跳过（`prepare_ocr_models.ps1:40-67,72-89`）；DLL 从 GitHub version release 下载，已有 DLL 即跳过（`prepare_ort_runtime.ps1:12-40`）。脚本内内容 hash/签名核验 `nothing found`。`build.rs:1-3` 仅 tauri_build。

## 关键不变量与需继续验证的假设

| 不变量/假设 | 当前建立它的代码与检查边界 |
|---|---|
| 返回翻译即有持久化记录 | workflow stage 写库；无记录则 Err，`translation_workflow.rs:204-244` |
| enrichment 不重复计次、不覆盖收藏 | workflow update=false；UPSERT 不写 is_favorite，`translation_workflow.rs:418-425`，`database.rs:324-346` |
| latest request 不再发布后续结果 | stage 前/await 后检查；检查与同步保存/emit 不是同一把原子锁，`translation_flow.rs:206-216`；跨入口是否需要统一 freshness 尚未定义 |
| loading 表示唯一进行中请求 | 图片入口检查 loading，文本/剪贴板 store 不做此入口检查，`translation.ts:121-151,202-205,235-238`；UI 层约束与并发覆盖须 tests/runtime 核实 |
| 设置保存失败不会产生多份不同状态 | 内存先更新、前端只在 invoke 成功后更新；除快捷键外回滚 `nothing found`，`app_state.rs:110-135`、`stores/settings.ts:96-117` |
| 日志 filename 一定在日志目录 | 仅 lexical starts_with，canonical/成员列表检查 `nothing found`，`logger.rs:63-74` |
| overlay 每一行是对应翻译 | 仅行数相等就 zip，不相等合并包围框，`translation_workflow.rs:72-125`；翻译服务保持顺序是外部假设 |
| OCR 可用等于可初始化 | status 只认模型文件；DLL/init/dict 错误另在 initialize 才见，`ocr.rs:84-107`、`native_ocr.rs:163-184` |
| 本地词典缺失会自动远程回退 | 启动提示如此（`lib.rs:615-616`），查询却返回 Err（`local_dictionary.rs:62-76`，`translation_flow.rs:88`）；保留两条事实供审计，不自行消解 |
| OCR profile 约定一致 | 文档允许 tiny/small/medium（架构文档:114），update 强制 native/small（`lib.rs:100-109`），native 仍识别 tiny/medium（`native_ocr.rs:403-408`）；实际产品期望待确认 |

## 测试命令与环境要求

本任务只登记，不声称通过。测试成员应记录实际 OS/Node/Rust 版本与退出码，使用临时/合成数据，避免启动真实配置的应用。

```text
npm run test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
npm run test:e2e
npm run verify:final
```

- `verify:final` 串行跑前端测试、Rust test、Rust check、前端 build，首错停止；不含 E2E/安装包验证（`scripts/verify-s06.mjs:59-83,96-110`）。
- Vitest 默认 node，发现 `src/**/*.spec.ts` / mount specs（`vitest.config.ts:14-18`）；组件文件可自行选择 jsdom，需核实实际测试覆盖而非只看文件名。
- Playwright 使用 tests/e2e、headless、127.0.0.1:4173 Vite dev server，可复用已有 server（`playwright.config.ts:3-14`）；不是原生 Tauri 进程测试。入口 `tests/e2e/translate-golden-path.spec.ts` 待测试成员检查 mock 范围。
- 文档要求 Node 18+、Rust stable、Tauri Windows 依赖、PowerShell（`README.md:207-239`）；此为文档声明，当前 lockfile 中工具的 engines 应另核实。Rust 含 Windows API、bundled SQLite、ppocr-rs 动态 runtime（`Cargo.toml:20-58`）。实际 Windows SDK/MSVC/WebView2 以及其他 OS 构建条件需测试者报告。
- OCR 实测/打包需要 small ONNX 模型与 onnxruntime.dll；DLL 准备脚本仅 Windows x64（`prepare_ort_runtime.ps1:18-20`）。词典构建需要 Python（`package.json:9`），数据来源/下载行为请安全成员进一步查看 `scripts/build_dictionary.py`，不要自动运行下载。
- Windows 非交互启动 npm 的 shell 宿主注意事项见 `docs/dev-startup.md:20-44`。开发运行可能写真实 app data、注册全局快捷键、自动 OCR 预热，不宜无隔离直接启动。

## 交接定位与未决问题

- architecture：检查 workflow/flow/domain/adapter 方向与文档漂移；特别是设置权限/规范化的分散点、lib.rs composition root、跨窗口事件所有权。
- security：验证自定义 IPC 可达窗口、日志文件边界、请求错误日志与密钥/文本暴露、Audio URL、native DLL/model 信任、实际资源合并与发布供应链。未访问任何实际秘密。
- runtime：验证并发取消和持久化原子性、settings 内存/磁盘失败语义、选区 guard 超时重入、UIA/剪贴板恢复、不同屏幕 DPI/非 Windows 编译分支、OCR 锁与状态含义。
- tests：运行基线并区分 Node unit/mount、browser mock E2E、真实 Windows native；确认依赖 engines、打包资源完整性和缺少资产时是否影响 Rust tests。
- 未决产品问题：是否正式支持非 Windows；是否需要跨所有入口统一 latest-wins；本地词典缺失应报错还是回退；profile 是否只允许 small；本地历史/日志保存范围和设置凭据存储的预期信任模型。
