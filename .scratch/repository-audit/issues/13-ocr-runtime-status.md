# 13 OCR 状态区分模型存在和引擎已初始化

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: runtime + frontend settings owner
Source: R6

## 顶部当前结论（2026-10-22）

**已关闭（resolved）**：生产生命周期seam与纯投影经稳定树 `native_ocr::lifecycle_tests` 6 passed、`ocr_contracts::tests` 6 passed、全量126 passed/0 failed、`cargo check --tests` exit0验证；负责人批准关闭。保留边界：Ready在build完成后、RUNTIME安装前有短窗口；failure仅按profile、progress全局；未验证RUNTIME锁中毒恢复；未跑真实模型/DLL/UI视觉。下方较早的“执行blocked”记录是关闭前历史，最终以本段及末尾证据为准。

## 根因与证据
`ocr.rs:84-107` model_dir.is_some就running=true/last_error=None；`native_ocr.rs:149-152,411-415`只查三个文件；真正build_runtime在`:163-184`可失败。`SettingsPage.vue:111-117`把running映射为success/运行中（`:308-316`触发状态查询）。triager重读函数确认。关闭预热未初始化、或模型/DLL加载失败后，仍可假健康。

## 严重性与反证
P2：诊断状态错误，不证明识别算法失败或所有OCR不可用。仅文件缺失分支提示正确不能反驳存在但无效模型。

## 范围 / 涉及文件
`native_ocr.rs`保存初始化生命周期/最近错误；`ocr.rs`汇总不触发初始化；`ocr_contracts.rs`及 `src/stores/settings.ts`状态类型、`SettingsPage.vue`如需新字段同步更新。模型可用/未初始化/初始化成功/初始化失败明确区分；保留运行中旧引擎与新profile失败的语义。

## 验收与回归
生产status seam+fake initializer测试：文件存在未初始化、初始化成功、注入失败后last_error、重试成功清错误、文件缺失、profile不同。状态查询不得初始化引擎/访问网络；UI分别显示状态。真实DLL/模型smoke是额外原生验证，当前资源环境阻塞不算通过。

## 非目标
不实现真正restart/销毁协议，不加载真实用户模型，不借机开放tiny/medium（21）。

## Comments
2026-10-22 当前复核状态（历史，已被末尾执行证据推翻）：**当时静态PASS，执行blocked，未关闭**。最终生产生命周期seam已独立于推理锁；initialized匹配profile/path，failure仅按profile，progress为全局。真实失败→阻塞重试Initializing→Ready、TEST_LOCK隔离、进度Drop清理已只读核对；tests当前受shell启动失败影响，不能将旧DTO测试通过视为最终生命周期版本已执行。Ready在build完成后、RUNTIME安装前有短窗口；panic测试仅证明标志清理，不证明生产锁中毒恢复。无真实模型/DLL/native验收。下方为此前实施历史，不覆盖此当前状态。
2026-10-22 执行证据回填：tests在稳定树（C:/translate-audit-target，测试子进程resources=[]）串行运行native_ocr::lifecycle_tests 6 passed、ocr_contracts::tests 6 passed，全量107 lib+4 dict+15 popup=126 passed 0 failed，cargo check --tests exit0；非旧91结果。上述静态待补项已由最终失败→阻塞重试回归覆盖。仍未执行真实模型/DLL与UI视觉；保留Ready发布短窗口、failure按profile、progress全局、poison恢复未验证等边界。能否关闭由负责人决定，triager不自行改Status。
2026-10-22 生命周期重写（替代下方旧 `LAST_INIT_ERROR` 四态实现）：`native_ocr.rs` 新增 `Lifecycle { initialized: Option<(profile, path)>, in_progress, last_failure: Option<(profile, error)> }` 与 `LIFECYCLE`；`initialize_locked<T,B>` 为生产/测试共用 seam，进入 build 前在同一短锁内置 `in_progress` 并清除同 profile 旧失败，`ProgressGuard` 的 Drop 在成功/失败/panic 展开时清标记（不 catch_unwind），失败保留旧引擎；`engine_snapshot_for(profile, model_dir)` 为纯投影，尺寸匹配要求 profile+path、failure 仅同 profile、progress 全局。`ocr_contracts.rs` 新增 `Initializing` 态、`initialized_profile`、`initializing`、`last_failure_profile`，`from_snapshot` 顺序为模型缺失→就绪→失败→初始化中→未初始化。`ocr.rs` `native_status` 改用 snapshot。前端 `settings.ts`/`SettingsPage.vue`/`settings.spec.ts` 同步五态。冻结前 triager 定点审出并已修复：多余右括号、`Option<&str>`/`Option<PathBuf>` 类型不匹配、需要 `&mut Lifecycle` 的访问器、`InitOutcome<T>` 泛型缺失、测试共享全局状态串扰（改 `TEST_LOCK` + Drop 恢复原状态）、失败后重试被旧 failure 抢先显示 Failed。测试：`native_ocr::lifecycle_tests` 6 项（含失败→阻塞 fake build 期间 Initializing→返回后 Ready、build panic 清进度、持 RUNTIME 查询不阻塞、失败不清健康旧引擎、retry 成功走真实 build 分支）与 `ocr_contracts::tests` 6 项在稳定树串行 6/6 passed，全量 126 passed/0 failed；tests 新树结果，不沿用旧 91。已知边界：Ready 在 build 完成后、RUNTIME 安装前有短窗口；`progress` 全局而非按 profile；failure 仅 profile 不记 path；panic 测试不覆盖 RUNTIME 锁中毒恢复；未运行真实模型/DLL 与 UI 视觉。
2026-10-22：采纳；状态接口改动由同一负责人端到端负责，不拆成互相等待的字段票。
2026-10-22：已实现。`ocr_contracts.rs`：新增 `OcrRuntimeState`（modelsUnavailable/notInitialized/ready/failed）、`OcrEngineSnapshot` 及纯函数 `OcrServiceStatus::from_snapshot`；移除 `running`。`native_ocr.rs`：初始化成功清空/失败记录 `LAST_INIT_ERROR`（仅诊断，不参与推理授权），`engine_snapshot` 仅在 profile 与模型目录都匹配时报告 initialized；状态查询用 try_lock，不触发初始化、不访问网络。`ocr.rs` 由 snapshot 汇总。前端 `settings.ts` 类型与 `SettingsPage.vue` 标签/详情按四态显示；`settings.spec.ts` 更新为新 DTO。验证：前端 settings.spec 经 tests 代验通过、全量 181 passed；Rust `ocr_contracts::` 未运行（bash add_item 故障），待单 filter 证据。未加载真实模型/DLL，未测 UI 视觉。
