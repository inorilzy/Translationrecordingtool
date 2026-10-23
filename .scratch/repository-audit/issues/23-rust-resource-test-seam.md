# 23 评估资源缺失下 Rust 检查与正式打包的分离

Type: task
Status: resolved
Severity: P2 (testability improvement, not confirmed product bug)
Blocked by: none
Owner suggestion: tests + Rust build maintainer
Source: tests.md Rust environment blocker
File lock（调度约束）：package.json 脚本改动交由 package/lock 单一 owner 合并（主负责人）；不改 `tauri.conf.json`、`build.rs`。

> 2026-10-22 状态变更（用户审批计划“23：验证入口通道”选定测试专用入口方案）：原 `research / needs-triage`，转 task/ready-for-agent，尚未派发。以下原文保留为历史。

## 证据与反证
cargo test/check --locked均exit101：Tauri build script缺 `resources/dictionary.db`，Rust测试未执行。README:222-233已要求准备词典/模型；triager核对文档，故不是无条件新checkout必须直接通过的产品缺陷。npm unit157与build通过不能覆盖Rust。

## 待评估 / 涉及文件
`src-tauri/build.rs`、`Cargo.toml`、`tauri.conf.json`、`tauri.ocr-native.conf.json`、`scripts/verify-s06.mjs`、README。先记录资源前置与经授权准备路径；再评估是否可让纯domain unit/check使用明确的非打包测试配置，而release仍严格校验资源。需要真实Tauri build语义证据，不能凭配置猜测。

## 验收与回归
产出可执行推荐与取舍：选择正式资产准备，或测试专用构建边界；至少用受控环境证明test/check进入真实编译/执行，明确test数量与exit；release缺正式资源必须失败。记录额外平台系统库条件。依赖代码编译问题出现后另立票，不伪造空dictionary.db/假模型让门禁绿。

## 非目标
不自动下载大资产、不启动用户应用、不禁用release资源检查、不把整个Rust重构为新workspace当默认方案。

## Comments
2026-10-22 本轮实施：新增 `scripts/rust-test.mjs`、`rust-test.test.mjs` 与npm入口。wrapper五项测试通过；本轮 `CARGO_TARGET_DIR=C:/translate-audit-target npm run test:rust -- --offline --locked` exit0，108 lib+4词典+15弹窗=127 passed（取消旧回退后测试数量下降，不沿用134）；`check:rust`编译完成。负向对照 `TAURI_CONFIG='{}' cargo check --offline --locked --manifest-path src-tauri/Cargo.toml` exit101，明确报缺resources/dictionary.db；正式配置未改。等待独立复核与最终冻结回归后关闭。
2026-10-22：环境阻塞及维护性研究；不是11等修复的语义前置，但其全Rust验收有此实际环境门槛。

## 2026-10-22 本轮选定方案与验收
- 新增窄 Node 测试 wrapper（路径实施时记录，建议 `scripts/` 下）及 package 脚本；仅对 Cargo 子进程设置 `TAURI_CONFIG` 覆盖 `bundle.resources=[]`，保留其他配置字段；传播子进程退出码与信号。
- 不修改正式 `tauri.conf.json` 或 `build.rs` 跳过生产检查；不制造空 dictionary.db/假模型。
- `CARGO_TARGET_DIR` 尊重调用者环境，不硬编码个人 C 盘路径。
- 回归：fake cargo 验证参数透传、环境继承、非法/缺失配置处理与退出码/信号传播。
- 反向门禁：不经 wrapper 的正式配置在缺资源时仍失败（原“release 缺正式资源必须失败”保留）。
- 真实 `cargo test`/`check --tests` 进入编译执行由唯一 Rust 执行者在 28 中记录实际数量与 exit；本票实施者不跑全量。

2026-10-22：按用户审批计划转 ready-for-agent；未实施。

## Answer
测试专用 wrapper 已通过 6 项测试，含真实 fake cargo CLI exit0/17；最终 Rust check 与 129 项测试通过，正式配置缺 dictionary.db 仍 exit101。负责人接受平台边界并批准关闭，见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；**Windows 真实 POSIX 信号传播 NOT RUN**，模拟返回路径不等于原生信号实测。上方 127/未实施等为历史。
