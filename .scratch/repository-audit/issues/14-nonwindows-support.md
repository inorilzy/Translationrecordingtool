# 14 确认非 Windows 支持后处理不可编译的截图分支

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: platform maintainer
Source: R7
File lock（调度约束）：独占 `screenshot.rs`、`screenshot/webview.rs`；如需改 `Cargo.toml` target 依赖，与 package/lock/Cargo 单一 owner 串行交接。

> 2026-10-22 产品决策（用户审批计划）：**仅 Windows 为正式已验证平台**。不删除非 Windows 实现；修复明确的 cfg 编译错误；其他平台无法执行 target 检查则标“未验证”，不伪造支持声明。原 `research / needs-info / P2 (conditional)`，转 task/ready-for-agent，尚未派发。以下原文保留为历史。

## 根因与证据
`screenshot.rs:203-218`使用未导入Screen；`:133-137`将app移入 `webview::select_and_capture_with_area(app)`再借用；`screenshot/webview.rs:47`确为按值参数。triager重读父/子模块，子模块import不会导出给父。README开发要求Windows；支持范围未明确。

## 严重性与反证
如支持非Windows，则P2平台分支编译回归；Windows cfg不激活，所以不能叫全产品不可构建。未执行cross target cargo check，静态错误不是已完成平台验收。

## 待答 / 涉及文件
维护者明确目标OS/target。若支持：`screenshot.rs`补cfg import和借用/clone，`screenshot/webview.rs`视签名方案调整，并建立对应target检查。若仅Windows：明确范围，决定是否封闭/移除闲置分支而非虚构支持。

## 验收与回归
记录支持决策与测试机器；支持分支cargo check成功且Windows不回归；遇系统库/资源前置缺失记录阻塞，不能用禁用代码代替支持验收。

## 非目标
不推广完整跨平台移植、不修未证实DPI问题、不将两处同一失维护平台分支拆成重复高优票。

## Comments
2026-10-22：保留needs-info，不自动实施；与10同文件需串行。

## 2026-10-22 本轮范围与验收
- `screenshot.rs`：修非 Windows 分支按值 move 后借用（计划记录约 `:122–123`，原分诊记录 `:133–137`，以实施时实际行号为准）；在正确 cfg 范围补 `Screen` 导入（计划约 `:179–195`，原记录 `:203–218`）；复核 `screenshot/webview.rs` 调用签名。
- 不凭推测迁移所有 Windows 依赖；只有确实阻碍目标编译的依赖才调整 Cargo target 配置。
- 验收：Windows `cargo check --tests` 与相关测试不回归（经 23 入口、唯一 Rust 执行者）；若可获得非 Windows target 工具链/系统库则执行 `cargo check --target ...` 并记录；不可得时该项标 NOT RUN / 未验证，不能把跨平台验收写 passed。README 支持声明由主负责人统一合并为“仅 Windows 已验证”。
- 原“支持分支 cargo check 成功”验收保留：若无法执行，本票可就“Windows 正式范围 + 静态修复”由负责人裁决，但非 Windows 编译成功不得被声称。

2026-10-22：按用户批准平台决策转 ready-for-agent；未实施。

## Answer
Screen cfg 导入与 AppHandle move 后借用已修；按 Windows 正式支持范围及已批准兼容回归关闭。Linux/macOS target NOT RUN，非跨平台编译成功。证据见 [最终验证](../final-verification.md)，此前状态为历史。

## Comments
2026-10-22 最终裁决：负责人依据 [最终 Windows Rust check/129 项测试与复核](../final-verification.md) 批准 resolved，保留非 Windows 未验证边界。
