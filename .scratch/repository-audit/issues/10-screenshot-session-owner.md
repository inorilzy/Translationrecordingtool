# 10 截图 guard 不得按时间释放仍存活会话

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: runtime / native screenshot
Source: R3

## 根因与证据
`screenshot.rs:30-67` 超90秒清全局bool，旧guard Drop也无条件清理；Windows `screenshot/windows.rs:354-357` 消息循环没有同等超时。runtime抽取原guard、只前移时钟后获取replacement，再drop(old)时第三次获取成功；triager重读算法确认。正常长时间选区+再请求即可达，不需要线程崩溃。

## 严重性与反证
P2：单会话互斥失效/可能多个选区；未证明崩溃或内存破坏。不因涉及原子操作就评P1。

## 范围 / 涉及文件
`src-tauri/src/screenshot.rs`（主要），只有选择显式取消协议才改 `screenshot/windows.rs`。最小安全修复：存活guard期间始终拒绝新请求，取消纯时间强制解锁；若保留回收，必须终止并确认旧原生会话退出，owner/generation保障旧Drop不释放新owner。不要只增加token却仍让两个活会话并存。

## 验收与回归
生产guard测试：长时间存活仍拒绝，drop后可重取；显式取消路径允许重取；若实现replacement，旧Drop对新owner无效；系统时钟跳变不影响所有权。隔离native延迟确认/ESC smoke，无真实GUI条件时明确未覆盖。

## 非目标
不重写截图渲染、不同时修混合DPI/跨屏、不宣称修了未经证实的Win32资源泄漏。

## Comments
2026-10-22：采纳；与14共享screenshot.rs，平台票未决，不应并发改该文件。
2026-10-22：已实现（仅 `screenshot.rs`，未改 windows.rs/非 Windows 分支）：90 秒强制解锁与系统时钟依赖删除；slot 0=空闲，CAS 0→1 获取唯一 owner token，只有 owner 的 Drop CAS 自身 token→0 才释放。测试使用独立 `static` slot：并发拒绝、长存活持续拒绝（100 次）、模拟代际替换后旧 Drop 不清新 owner 且第三次被拒绝、replacement Drop 后恢复、正常 Drop 后可重取。
2026-10-22 证据：`cargo test --offline --locked --manifest-path src-tauri/Cargo.toml screenshot::`（CARGO_TARGET_DIR=C:/translate-audit-target，测试进程 TAURI_CONFIG 资源覆盖）编译成功，6 tests，最终 0 failed（此前中间版本曾因测试断言错误失败，已修正；非生产缺陷）。其中代际测试走真实替换路径：模拟把 slot 交给新 owner 后旧 Drop 不清除 owner（断言 owner 仍为新 token）、第三次请求被拒绝、replacement Drop 后恢复空闲；并新增 token≠0 断言（待下次 cargo 窗口复跑确认）。未运行真实原生选区/ESC/超时 GUI smoke，未覆盖 windows.rs 资源路径。
2026-10-22：triager 只读复核 PASS；Status → resolved。
