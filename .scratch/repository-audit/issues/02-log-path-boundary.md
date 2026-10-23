# 02 日志读取约束到允许的日志文件

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: security / Rust IPC
Source: SEC-02

## 根因与证据
`src-tauri/src/logger.rs:57-74` 对 join 后路径仅 starts_with，未消除 ParentDir；`lib.rs:768` 注册命令。security 在 Windows 合成目录 Rust 测试读取同级 synthetic 文件成功；triager 重读实际函数确认检查不具备规范化语义。普通 LogsPage 只传枚举文件，恶意 IPC 调用者才可传穿越路径。应用 commands 未设 application ACL，不能依窗口 plugin capability 推断隔离。

## 严重性与反证
P2：确定突破日志目录契约、扩大受损 WebView 的读文件能力；没有证明不可信远端脚本入口，不评远程泄露/P1/RCE，不突破账户权限。

## 最小范围 / 涉及文件
`src-tauri/src/logger.rs`：仅正常单一 basename、允许日志扩展；拒绝绝对路径、前缀、分隔符、父目录/子目录；canonical 根/目标后验证普通文件及归属。错误不得返回目标文件正文。

## 验收与回归
生产校验函数表测：合法日志、缺失文件、../、..\\、绝对路径、UNC/盘符前缀、子目录、错误扩展。临时目录里外文件内容均用 synthetic，越界零读取。支持时补符号链接/reparse point，明确 TOCTOU 的本地并发写威胁仍需另评；单次 canonicalize 不声称防任意竞争。隔离 IPC 窗口测试需 native 环境后运行。

## 非目标
不将 CSP、app ACL、密钥库混入此票；不扩为文件浏览器，不访问真实用户文件。

## Comments
2026-10-22：采纳；保持 IPC 攻击前置条件，不宣称已有远程利用。

2026-10-22（security，实施阶段）：`logger.rs` 的 `read_log_file` 现调用纯函数 `read_log_file_in_dir`：仅接受单一 Normal 文件名组件，拒绝 `/`、`\`、`:`（含 ADS）、绝对/UNC/盘符相对路径和子目录，扩展必须严格为 `.log`；canonicalize 后要求根已存在且为目录、目标父目录等于根且为普通文件；非法与不存在统一错误，读取失败不返回目标路径或正文。同文件合成临时目录表测覆盖合法日志、缺失、`../`、`..\`、绝对/UNC/extended 前缀、盘符相对、子目录、大小写/尾点/空格/ADS 变体、目录目标和目录外文件；Unix 外部 symlink 用例按平台编译。TOCTOU/reparse-point 并发写未纳入本票保证。

验证：`cargo test --locked --manifest-path src-tauri/Cargo.toml logger::tests`（CARGO_TARGET_DIR=C:/translate-audit-target，TAURI_CONFIG 覆盖 resources=[]）1 passed exit0；同一隔离目标全量 `cargo test` 91 passed exit0；`cargo check --tests` 覆盖资源 exit0；无覆盖的普通 `cargo check` 仍因缺少 `resources\dictionary.db` exit101。triager 已独立复核并实跑 logger::tests 1 passed。未读取真实日志目录，未改 CSP/ACL/密钥库，未提交。

2026-10-22（security，关闭）：triager 终审 PASS；负责人确认以本方实跑 + triager 独立执行证据关闭。最终 diff 仅涉及 `logger.rs`。Status 置为 `resolved`；IPC 窗口级集成、Windows reparse-point 与 TOCTOU 竞争仍不在本票已验证范围。
