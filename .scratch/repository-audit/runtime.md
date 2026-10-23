# Rust 运行时与数据正确性审计

基线：`d72a76967f3f79d0e51eae4c049889d32f2e9a42`。日期：2026-10-22。状态：供 triager 独立复核，非已修复声明。

已读取 `.scratch/repository-audit/context.md`、项目 AGENTS 及 docs/agents 三份规范，并沿下列入口读取实际调用链。未启动应用、未读写真实剪贴板/历史/设置、未调用 OCR 推理或外部接口、未创建子代理、未修改源码。仅在系统临时目录编译运行抽取的纯函数及模拟依赖，运行后删除临时目录。

## 结论摘要

无已验证 P0/P1。7 项 P2 候选，其中 R1/R2/R3/R4 有隔离执行证据，R5/R6 为完整源码链证据，R7 是明确的条件编译源码错误但产品跨平台支持范围待确认。不要将纯函数测试称为真实 Windows UI 端到端复现。

| ID | 优先级 | 问题 | 证据 |
|---|---|---|---|
| R1 | P2 | 划词回退会丢失原剪贴板非文本格式 | 抽取实际恢复函数 + 合成内容执行 |
| R2 | P2 | 恢复旧剪贴板不检查所有权，覆盖并发复制 | 抽取实际恢复函数 + 合成内容执行 |
| R3 | P2 | 截图 stale guard 不终止旧会话，旧 Drop 解锁新会话 | 原样 guard 隔离 Rust 执行 |
| R4 | P2 | 设置持久化失败仍发布运行时新配置 | 原样 theme 更新函数 + 注入保存失败 |
| R5 | P2 | settings.json 原地截断写入，异常中断可丢失整个配置 | 完整写入/加载/启动调用链 |
| R6 | P2 | OCR 状态将模型文件存在误报为运行正常 | status → model_status → 文件检查及 UI 链 |
| R7 | P2 / 平台范围待确认 | 非 Windows 截图分支无法编译 | 未导入类型 + move 后 borrow；未交叉编译 |

## R1 — 非文本剪贴板在划词回退后不能恢复

- **位置**：`src-tauri/src/shortcut_handler.rs:315-331,295-312`；`src-tauri/src/clipboard.rs:1-14`。
- **可达链**：全局快捷键 `register_shortcut_handler:33-36` → `handle_shortcut:63-67` → UIA 失败/空值 `select_text_source:271-290` → `copy_selection_and_read_clipboard`。备份只调用 `read_text().ok()`；Ctrl+C 改写剪贴板后仅 `Some(String)` 才写回文本。
- **复现/证据**：原样抽取 `read_selection_and_restore_clipboard`，只把 tracing warn 替为无副作用宏。合成剪贴板初始 `original-image`，传入 `previous_clipboard=None` 模拟真实 `read_text` 不支持图片；读取闭包模拟 Ctrl+C 后内容为 `selection`。结束后断言仍为 `selection` 成功。输出：`REPRO: non-text previous content is not restored`。真实系统剪贴板完全未接触。
- **影响**：用户先复制截图/文件，再对不支持 UIA 的应用划词，即可能丢失之前剪贴板内容；带 HTML/RTF 等多格式文本的内容也只恢复纯文本。不是仅日志或错误提示问题。
- **最小修复范围**：剪贴板 fallback adapter 需要明确支持原格式快照/恢复；不能完整备份时不要无提示破坏性 Ctrl+C。格式保真策略可与 R2 共用快照对象，但此问题与并发所有权不同。
- **未验证**：没有枚举真实 OS 各格式；图片/文件具体插件错误文案未执行。业务需要决定是否允许明确告知用户的降级。

## R2 — 剪贴板恢复无所有权校验，会覆盖之后的用户复制

- **位置**：`shortcut_handler.rs:295-312,315-331`；`clipboard.rs:29-85`。
- **可达链**：每次按键均 `spawn` 独立任务（`shortcut_handler.rs:33-36,62-67`），没有覆盖“备份—模拟复制—等待—恢复”的互斥。sequence number 仅用于判断是否发生过更新（`clipboard.rs:59-68`），不记录本请求取得的更新序列供恢复前比对。
- **复现/证据**：上述实际恢复函数第二次调用，备份 `old`；read_selected 闭包模拟成功读取 selection 后用户再复制 `new-user-copy`；restore 闭包写回输入值。函数成功返回，但最终值断言为 `old`。输出：`REPRO: intervening clipboard write overwritten by old snapshot`。这证明恢复函数无条件覆盖，未宣称已跑真实键盘时序。
- **具体交错**：A 保存旧值 O；A 模拟 Ctrl+C；B（用户/另一快捷键任务）更新剪贴板；A 的 wait 可能把 B 当目标文本，随后无条件写回 O。即使等待超时也调用恢复，仍有覆盖机会。
- **影响**：正常快速复制或连续划词时丢失用户的新复制内容，并可能翻译与选择不同的文本。请求 ID 过滤在之后翻译/弹窗阶段，不保护此前剪贴板副作用。
- **最小修复范围**：序列化本应用的整段 fallback；快照携带序列/ownership token，恢复前仅当剪贴板仍属本次模拟复制时恢复，用户后续复制优先；键盘模拟失败应显式处理。单加 mutex 不能解决外部用户复制。
- **未验证**：没有真实双按键/外部应用实验；如何可靠识别模拟复制完成须结合 Windows 剪贴板 API 实现评估。

## R3 — 90 秒 stale 回收破坏截图单会话互斥

- **位置**：`src-tauri/src/screenshot.rs:19-67,120-147`；`src-tauri/src/screenshot/windows.rs:277-289,297-370`。
- **可达链**：截图快捷键 `shortcut_handler.rs:109-112` 或 IPC `lib.rs:461-466` → spawn_blocking → guard → Windows 原生消息循环。Windows `GetMessageW` 循环（`:354-357`）没有 90 秒自动退出；单纯长时间未确认选区不等于线程已经终止。
- **复现/证据**：原样提取 `SELECTION_IN_PROGRESS` 至 guard Drop 的源码到临时 Rust 程序。步骤：获取 old guard；仅把全局开始时间设置为 now-90001ms（跳过真实等待，不改算法）；取得 replacement guard；drop(old)；在 replacement 仍活着时再次 try_acquire。第三次获得成功。输出：`REPRO: third session acquired while replacement guard remains alive`，rustc 与程序退出码均 0。
- **影响**：旧原生窗口仍在时可再创建一组截图窗口；旧会话结束又无条件清除新会话占用标记，后续任何请求可继续重入。固定窗口类名还会让两会话共享 class 注册/注销资源。未证明崩溃，不应升级为已确认内存破坏。
- **最小修复范围**：不要仅按时间强行释放仍存活 owner。原生选择线程先明确取消并确认结束，再允许新会话；guard 使用 generation/owner token，Drop 只能释放自己的会话。时间使用单调时钟。回归至少覆盖超时替换后旧 Drop 与新请求并发。
- **未验证**：未实际显示选区窗口，未验证 Win32 类注册失败后具体视觉表现。

## R4 — 保存失败但 canonical settings 已生效

- **位置**：`src-tauri/src/app_state.rs:96-135,152-161`，尤其 `:110-123` 与 `:131-135`；`src/stores/settings.ts:96-117,137-166`。
- **可达链**：设置页面 → store await invoke → `lib.rs:80-93,303-317` → helper 先改 `Arc<RwLock<SettingsRecord>>`，释放锁后 `persist_managed_settings:83-88` → `settings.rs:127-136`。失败没有回滚；store 只有 await 成功才更新。翻译下一请求从 canonical 读配置（`translation_workflow.rs:166-178`），主窗口关闭直接读 enable_tray（`lib.rs:667-679`）。
- **复现/证据**：原样抽取 `update_and_persist_theme`；AppHandle 为占位类型；RwLock wrapper 用 std RwLock 同步读写；persist stub 总返回 `Err("injected disk failure")`。输入 light→dark，断言函数 Err 且内存 dark 成功。输出：`REPRO: save returns error but runtime theme changed to dark`。真实磁盘故障条件可为目录无写权限/文件锁定；未修改真实配置权限。
- **影响**：UI 表示保存失败且保留旧值，后台却使用新 provider/key 或新 tray 行为。后续其他字段保存还可能顺带持久化这次“失败”的变更。快捷键命令有独立补偿，不可把它当作所有字段都安全的证据。
- **最小修复范围**：构造候选 snapshot，成功持久化后再提交 runtime；更新/保存的事务边界需串行化避免后续 lost update。R5 的原子文件替换应先完成或与本修复统一为设置 repository。
- **未验证**：无真实 Tauri invoke 故障注入；并发设置命令是否受 Tauri UI 线程串行调度影响未定，因此未以并发丢写作为本项成立前提。

## R5 — 设置写入原地 truncate，配置损坏后静默回退默认值

- **位置**：`src-tauri/src/settings.rs:127-136,112-124`；`src-tauri/src/lib.rs:565-569`。
- **证据**：全量 JSON 通过 `fs::write(&settings_path, content)` 写到最终文件，没有临时文件/原子替换/备份恢复。标准库 fs::write 对已有文件先截断；写入途中的 I/O 错误/进程终止可留下空或不完整 JSON。下次 `load_settings` serde parse 返回 Err，启动捕获错误后使用 `PersistedSettings::default()`。
- **影响**：保存任一字段时的异常中断可使 provider 配置、快捷键、主题等全部丢失到默认状态；与 R4 的“正常返回 Err 后内存仍改变”是独立故障。已有快捷键恢复分支无法恢复已截断的磁盘文件。
- **最小修复范围**：同目录临时文件完整写入、必要同步、平台正确的原子替换；保留最后有效配置或显式恢复。回归用临时配置目录与受控写失败，不操作真实设置。
- **验证边界**：源码/标准库文件语义证据；未杀进程、模拟断电或填满磁盘，不声称已测真实崩溃恢复。

## R6 — OCR 服务状态只看文件，失败后仍可显示“运行中”

- **位置**：`src-tauri/src/ocr.rs:76-107`；`native_ocr.rs:149-152,368-415`；`src/views/SettingsPage.vue:111-117,308-316`。
- **可达链**：UI refresh → store getOcrServiceStatus（`settings.ts:195-201`）→ `lib.rs:418-423` → native_status → model_status → has_model_subdirs。三个文件 `is_file()` 就令 running=true、last_error=None；没有检查 RUNTIME 是否初始化或上次 init 是否失败。真实 init 仍可能在词典生成、DLL 加载和 `init_models_no_angle` 失败（`native_ocr.rs:163-184`）。
- **复现步骤（未调用 native）**：隔离资源目录放 det/inference.onnx、rec/inference.onnx、rec/inference.yml 三个空文件，状态逻辑会判可用；初始化需要解析有效模型，无法因 is_file 即保证成功。更常见情况：关闭启动预热，模型文件完整但 engine 尚未初始化，UI 同样显示“运行中”。
- **影响**：OCR 实际不可用时用户仍得到 success/运行中且 last_error=None 的诊断，影响故障定位与可用性判断。不是证明 ONNX 推理本身错误。
- **最小修复范围**：分离 modelsAvailable、initialized、lastInitializationError；status 读取 runtime 生命周期，不应为健康检查启动收费/外部请求。UI 按实际状态展示。
- **关联待确认**：`restart_ocr_service` → `ocr::restart:72-74` 实际仅 ensure；`native_ocr.rs:138-141` 同 profile/path 直接复用，不销毁重建。没有 UI 可达重启按钮证据，单列为契约问题，不另报已确认用户故障。poisoned std Mutex 也无法由此 restart 恢复；未触发真实 panic。

## R7 — 非 Windows 截图分支编译错误（范围待确认）

- **位置**：`src-tauri/src/screenshot.rs:133-137,203-218`；`screenshot/webview.rs:12,47`。
- **证据一**：父模块 `capture_area_as_png_data_url` 引用 `Screen::from_point`（:211），父模块未导入 Screen；子模块 webview 的 `use screenshots::Screen` 不向父模块导出名字。
- **证据二**：`webview::select_and_capture_with_area(app)` 按值消耗非 Copy 的 AppHandle，然后 `restore_main_window(&app)` 再借用；该 helper 签名明确取 AppHandle，不是引用。
- **影响**：非 Windows cfg 激活时出现 unresolved type / borrow of moved value，无法构建。Windows 条件编译不会检查这两个分支。
- **最小修复范围**：修正父模块 cfg import、传参所有权（借用或 clone），至少增加一个支持平台的 cargo check；若产品明确仅 Windows，删除/封闭无效平台承诺并明确支持范围。
- **未验证**：未安装 cross target、未执行 Linux/macOS cargo check；tests 成员确认此次不覆盖其他 target。建议 triager 根据正式平台支持范围决定 ready-for-agent 或 needs-info，而非将整个 Windows 产品标为不可构建。

## 未升级为确定缺陷的候选及覆盖边界

1. **取消/锁**：已读 `translation_flow.rs:53-239`、`translation_workflow.rs:166-337`、OCR spawn_blocking 与双 Mutex。取消是合作式 freshness，不会中断已开始的网络/推理；OCR holding runtime→engine，未发现反向锁序形成的确定死锁。旧请求排队消耗资源是事实，但无响应时延指标/压力证据，未报 P1。检查 cancellation 与同步 DB save 存在竞态窗口，但取消不能撤销此前已展示/持久化本地结果也可能是设计，需先定义一致性要求。
2. **数据一致性**：UPSERT 单条语句保持 favorite，enrichment 不加 access_count（`database.rs:324-346`）。写入后再独立 SELECT（`:368-374`）没有 transaction，两个同 key 写入可在间隙改变返回记录；未执行并发 SQL 实验，未验证产品是否要求按请求而非“最新同 key”返回，暂不立项。历史/收藏跨窗口缓存失效由 architecture 主报，避免重复。
3. **原生资源错误路径**：`screenshot/windows.rs:230-248,395-404` 在部分 HWND 创建失败时裸数组不会 DestroyWindow；`:306-320,339-342` 提前返回跳过类/brush 清理。成功路径有 Drop 和清理；资源耗尽或 Win32 创建失败条件未注入。建议独立故障注入后决定票据，未声称普通操作必泄漏。
4. **popup ready**：`popup_window.rs:363-383` check-then-once 注册存在 ready 事件恰在两者之间到达的窗口；常规预热降低触发概率。未建立可控 Tauri 事件调度证据，保留候选。
5. **快捷键**：已读注册、unregister、保存失败补偿 `lib.rs:136-299`。old_shortcut 信任前端、字符串比较未规范化、unregister 错误忽略可能导致状态漂移，但未真实注册全局快捷键验证，不单独高置信立项。
6. **词典缺失回退**：交给 architecture 核实产品契约，避免与其重复。非 Windows 截图跨屏裁剪、混合 DPI、overlay 销毁到 GDI capture 的时序未真实 UI 验证。
7. **OCR**：未加载真实 DLL/模型、未推理、未测试大图内存压力、未验证初始化 panic。读取路径/锁/状态代码不能替代原生运行测试。

## 隔离测试记录与复现方法

环境：Windows shell；`rustc 1.96.0 (ac68faa20 2026-05-25)`；`Python 3.14.5`。

方法：Python 从源码按函数边界提取文本到 `tempfile.TemporaryDirectory(prefix="runtime-audit-")`，调用 rustc 生成该目录下 exe，再执行，最后删除。没有修改仓库测试/源码，不加载项目真实 AppHandle。R1/R2 仅 RefCell<String> 模拟剪贴板；R3 是源码 guard 原样；R4 使用 std RwLock wrapper 与失败 stub。

- guard 测试初次因 shell 把 Rust 宏 `!` 转为 `\!` 而编译失败（非项目错误）。调整脚本通过 chr(33) 构造宏字符后，rustc=0、exe=0，打印 R3 复现行。
- clipboard 恢复测试 rustc=0、exe=0；仅 warn stub 导致 unused variable 警告，打印 R1/R2 两行。
- theme 更新测试 rustc=0、exe=0；仅临时 stub 可见性警告，打印 R4 行。
- 全仓 Rust test 由 tests 成员执行；其报告当前卡在 `resources/dictionary.db` 缺失，不能据此声称本审计通过全仓测试。无需重复启动或访问真实 app data。

建议交接顺序：R5 → R4（文件原子提交后再定义运行时事务）；R1/R2 可由同一剪贴板 adapter 改造但分别保留验收；R3 独立；R6 独立；R7 先确认平台范围。最终严重性与工单取舍由 triager 复核。
