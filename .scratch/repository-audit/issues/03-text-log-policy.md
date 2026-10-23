# 03 决定并落实翻译全文日志的隐私边界

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: security（本轮已派发）
Source: SEC-03
File lock（调度约束，非 Blocked by）：03 先于 09 写 `shortcut_handler.rs`；03 先于 20 写 `translation_flow.rs`。

> 2026-10-22 状态变更：原 `research / needs-info / P2 (proposed)`。用户已明确审批计划 `vectorized-scribbling-puppy.md` 中的隐私默认决策，转为 task 并派发 claimed。原文以下保留为分诊历史。

## 根因与证据
`translation_flow.rs:80-85` 默认 info 输出完整 text，`shortcut_handler.rs:69` 输出所选文本，`logger.rs:11-24` 最多七个日志文件并输出 stdout。普通输入即可达。triager 重读 flow/logger 确认；实际日志未读取。

## 严重性与反证
P2 候选：历史之外多一份容易分享的文本副本，含私密内容时有隐私风险；是否允许诊断全文、用户知情范围未明确，不能假定违规或已泄露。debug 改级不是脱敏证明。

## 待答问题 / 涉及文件
产品明确默认不得留全文，以及是否需要单独显式 opt-in。拟涉及 `src-tauri/src/translation_flow.rs`、`shortcut_handler.rs`、`translator.rs`（debug 响应）、`logger.rs`；若需设置 UI 应另列批准范围，不自行加设置。

## 验收与回归
记录决策、保留范围和支持日志分享边界后再转 task/ready-for-agent。若默认禁全文：默认日志只保留 request/provider/长度/耗时；正常/失败/划词 synthetic marker 在输出中均不存在；审查 debug opt-in 文案，不用短词 hash 充当匿名化。明确真实日志集成是否执行。

## 非目标
不删除用户历史或现有日志，不推断法律责任，不因本票修改密钥存储。

## 产品决策（2026-10-22，用户审批计划）
- info/debug 两级**均不**记录原文、译文、完整 provider 响应、带敏感查询的 URL。
- 保留诊断：类型、字符数、耗时、请求标识、provider、状态/错误类别。
- **不增加**敏感日志开关或 opt-in（原“是否需要显式 opt-in”问题答复为否）；不新增设置 UI。

## 本轮实施范围与验收（在原验收基础上具体化，不放宽）
- 审查并删除内容性日志：`translation_flow.rs:80–85`、`shortcut_handler.rs:69`、`translator.rs`（含 debug 响应）、`logger.rs` 错误路径。
- 回归：用 synthetic marker 文本覆盖正常翻译、provider 失败、划词路径，在 info 与 debug 输出中均不出现 marker；诊断字段仍存在。不用短词 hash 充当匿名化。
- 禁止读取真实日志或凭据；真实磁盘日志集成是否执行须如实记录，未执行写 NOT RUN。

## Comments
2026-10-22：事实成立，政策需求待维护者确认，不计已确定产品缺陷。
2026-10-22：用户审批计划确定隐私默认（见上），转 task/claimed，已派发 security。尚无实施或执行证据，待独立复核后才可 resolved。

## Answer
info/debug 内容日志及有道码、serde Display 泄漏已修；synthetic marker 回归及二次独立复核通过，负责人批准关闭。此前状态记录为历史。证据见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行证据与独立复核](../final-verification.md)，新树 Rust 129 项通过；不冒称真实磁盘日志/原生 GUI 验证。
