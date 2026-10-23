# 01 Google 请求错误不得包含 API key

Type: task
Status: resolved
Severity: P1
Blocked by: none
Owner suggestion: security / Rust provider adapter
Source: SEC-01

## 根因与证据
`src-tauri/src/translator.rs:255-275` 把 key 编码进 URL，send 错误直接 Display 后同时 error! 与返回；`logger.rs:19-24` 输出文件及 stdout。triager 独立读取锁定 reqwest 0.13.2 crate `src/error.rs:267-268` 确认 Display 完整 URL；security 核实 client 超时/请求错误附 URL。Google 配置非空 key 且连接失败/超时即达，不需要 XSS。详见 ../security.md SEC-01。

## 严重性与反证
P1：普通故障将可用于调用计费 API 的凭据复制进可分享诊断日志，属严重敏感数据风险；不是因代码风格或“应尽快修”。尚未证明日志外传、费用损失或真实密钥泄露，不是 P0。锁定版本 JSON decode 不附 URL，不能声称全部解析错误都泄露。

## 最小范围 / 涉及文件
修改 `src-tauri/src/translator.rs` 的 Google 错误处理（可在同文件测试模块新增真实 formatter 单测）。请求错误格式化前移除 URL，返回可诊断但无凭据的信息。逐一检查 Google send / decode / service error message 出口，若服务 message 包含假 key，禁止原样回显；用假 key 核验，不假设远端永不回显。

## 验收与回归
- 生产错误处理路径的单测使用假 key（含需 URL 编码字符）和附 URL 的请求错误，输出不含原始/编码 key 或 `?key=`；不得只测另抄一份 formatter。
- Google 三个错误出口均有含假 key 的负面测试；服务返回异常信息不能绕过脱敏；错误仍可区分请求失败与服务失败。
- 有隔离 Tauri 测试环境后捕获实际日志/stdout及 IPC 返回作集成断言；未运行必须标 blocked，不把 formatter 单测称为集成通过。
- 不调用真实 provider，不读取用户配置/日志。Rust 全门禁受资源前置条件限制，见 23。

## 非目标
不迁移密钥库、不重写 provider、不增加 CSP/ACL；上述不是本票依赖。历史日志的暴露评估/可能换钥由维护者依据实际情况决定，审计未读取它们。

## Comments
2026-10-22：triager 采纳并独立核对锁定依赖，尚未修复。

2026-10-22（security，实施阶段）：按负责人转达的用户授权，已在 `translator.rs` 修改生产路径：send/decode 错误统一先 `without_url()` 再格式化；服务错误对象不保留不可信 message/details，输出只含 HTTP 状态。新增4个同文件回归测试调用实际生产 formatter/service_error，以含需 URL 编码字符的假 key 验证原始/编码值均不外泄。reqwest 错误夹具为不发请求的 builder error 附 URL，测试不是实际网络 send/decode 或日志/IPC 集成。

验证：`git diff --check` 退出0；`cargo test --offline --locked --manifest-path src-tauri/Cargo.toml google_error_tests` 退出101，Tauri build script 报 `resource path resources\\dictionary.db doesn't exist`，测试尚未运行。未更改资源配置或采用复制源码 harness 绕过。已交 triager 独立复核。

2026-10-22（security，隔离目标复跑）：使用 CARGO_TARGET_DIR=C:/translate-audit-target 且仅在该次命令设置 TAURI_CONFIG 覆盖 resources=[]（未改仓库配置、未造发布资源），`cargo test --locked --manifest-path src-tauri/Cargo.toml google_error_tests` 4 passed exit0；同目标全量 `cargo test` 91 passed exit0（lib 82 + local_dictionary 4 + popup_window 15）；`cargo check --tests --offline --locked` 覆盖资源 exit0；无覆盖的普通 `cargo check` 仍 exit101，原因为 `resource path resources\\dictionary.db doesn't exist`。首轮 logger 测试曾因并发写作者使 `app_state.rs` 暂时缺少 `SETTINGS_COMMIT_LOCK` 而编译失败，未归因本方改动，随后复跑通过。真实 send/decode 网络路径及实际日志/stdout/IPC 集成仍未执行，生产日志集成仍 blocked；triager 已独立实跑本组 4 个测试。

2026-10-22（triager 终审）：PASS。非阻塞缺口：日志/stdout/IPC 仅为 formatter 级 stub 覆盖，未做真实网络失败 smoke；修复未涉及前端错误消费者。关闭前需负责人在单一稳定树串行核验 `cargo check --tests` 与全量 `cargo test`（空资源 target）。

2026-10-22（security，关闭）：负责人确认 triager 独立实跑证据充分，无需本方再次重跑即可关闭；用户已批准缓存清理，后续 cargo 待 tests 回报缓存重建。最终 diff 仅涉及 `translator.rs`；按审查意见 Status 置为 `resolved`，非阻塞缺口保留在上一条，不宣称真实网络/日志 IPC 集成已覆盖。
