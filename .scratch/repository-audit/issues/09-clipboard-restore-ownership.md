# 09 恢复剪贴板前验证本次复制所有权

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: runtime / Windows clipboard adapter (same owner as 08)
Source: R2
File lock（调度约束，非 Blocked by）：等 03 完成 `shortcut_handler.rs` 日志改动后再写该文件。

> 2026-10-22 产品决策（用户审批计划 `vectorized-scribbling-puppy.md`）：**取消自动 Ctrl+C 回退**。UI Automation 无法获取选文（失败/空值）时，提示用户手动复制后使用现有剪贴板翻译；保留主动剪贴板翻译、截图 OCR 与正常 UIA 路径。这是用户批准的功能范围变更，**不是**继续用 owner/PID/sequence 启发式修补；不再宣称能安全回滚用户剪贴板。原状态 ready-for-agent（PARTIAL），本次派发置 claimed。以下原文与 PARTIAL 记录保留为历史。

## 根因与证据
`shortcut_handler.rs:295-312` 无条件恢复旧值；每按键独立spawn，`:315-331`没有整个fallback事务互斥。`clipboard.rs:59-68` sequence仅判是否更新，不校验恢复所有权。runtime真实函数注入用户后续复制，最终被旧值覆盖；triager重读确认。与08不同：即便纯文本保真仍存在此丢失。

## 严重性与反证
P2：短窗口并发复制内容丢失/错选文本，非所有数据损毁。单加应用mutex不能防外部用户复制；后续popup请求ID过滤不能撤销此前剪贴板写入。

## 范围 / 涉及文件
`clipboard.rs`、`shortcut_handler.rs`。串行化本应用整个fallback，成功复制捕获稳定sequence/owner；恢复前在操作系统可保证的原子临界区检查所有权，只在仍属于本次操作时恢复。无法判定则保留新内容、明确失败，不以过期sequence覆盖用户数据。

## 验收与回归
可控调度测试：A/B连续fallback；复制后用户更新；读取失败/等待超时后用户更新；比较到写入之间外部更新；正常恢复。用户后写内容始终胜出，应用请求不互相当作目标。真实Windows序列与临界区实验单独记录；模拟测试不能证明OS无竞争。

## 非目标
不统一全部翻译latest-wins；不重复08的格式策略，不实现通用剪贴板管理器。

## Comments

2026-10-22 第二轮冻结树历史结论（已被末尾最终裁决取代）：**PARTIAL，保持ready-for-agent，不关闭、不拆票弱化验收**。原根因段为修复前证据；下列记录区分已修部分与仍未满足部分。

- 已修/静态PASS：`clipboard.rs:176–208`在同一OpenClipboard guard内检查备份前后sequence相等且非0，将其保存在快照；`shortcut_handler.rs:317–318,349–352`注键前校验快照，过期时零注键、零恢复。`clipboard.rs:254–266`在同一OS guard内完成token比较→EmptyClipboard→全部SetClipboardData；捕获后普通新写导致sequence/owner变化时拒绝恢复。`shortcut_handler.rs:296–304`串行化整个fallback；无token的初始化/注键/读取/超时失败不恢复；`:335–340`传播注键错误并始终尝试Release，不保证Release失败时物理Ctrl已释放。
- 未满足：检查快照至注键仍存在外部写入窗口；`clipboard.rs:215–238`的sequence/owner/PID只作启发式拒绝过滤，无法区分同目标进程主动/延迟写入与本次Ctrl+C。首次成功捕获token前用户新写仍可能被误认并回滚，故原票“用户后写内容始终胜出”尚未全面满足。u32序列回绕/同token重现亦不在绝对保护保证内。不采用已撤回的Original/Modified“修订授权”、删除PID或覆盖snapshot方案。
- 第二轮执行证据（tests成员提供，非triager重跑）：`clipboard::tests`14 passed、`shortcut_handler::tests`11 passed、`cargo check --tests` exit0，全量115 lib+4 dict+15 popup=134 passed、0 failed、0 ignored、0 skipped，四条命令均exit0。第一轮13/9及131全量仅为旧冻结树历史，不替代本轮证据。
- 测试边界：生产序列helper覆盖稳定/变化/零/缺值，过期路径用合成键计数和restore不可达断言。`shortcut_handler.rs:500–534`为channel协调的真实A/B线程，调用生产serialized_clipboard seam，确认A持锁时B try_lock失败及A restore先于B copy；未强制B阻塞lock调用一定先于A释放。其他所有权比较测试仍为合成token/std Mutex，均不触真实剪贴板或注键，**真实Win32临界区/序列/往返未验证**。既有延迟渲染阻塞、部分Set失败不能回滚的08边界保留。
- 本次定点复核未发现新增已确认P0/P1；这不是全仓安全保证。负责人要求保留本票原验收及残余风险，状态和23票统计不变。
2026-10-22：采纳；排在08之后属写文件所有权排序，不是Blocked by边。

## 2026-10-22 本轮修订范围与验收（取代上方“范围”中的启发式恢复方案；原“用户后写内容始终胜出”目标通过移除自动注键满足，不放宽）

决策与理由：自动 Ctrl+C 注键本身即可覆盖用户剪贴板，任何 sequence/owner/PID 比较都无法可靠归因（见上方 PARTIAL 记录），因此按用户批准从生产路径移除该回退。“不恢复但仍发 Ctrl+C”不算修复。

范围：
- `shortcut_handler.rs`：选文获取只走 UIA；UIA 失败/空值时复用现有 popup 错误展示，提示“请手动复制后使用剪贴板翻译”；删除对自动复制回退的可达调用。
- `clipboard.rs`：仅删除确认只被该回退使用、已无调用的实现；保留主动 read/write 及其他合法调用，不大规模清理。
- 用户文档记录功能兼容性变化（README 由主负责人统一合并，本票实施者不直接改 README）。

验收：
- 生产入口 seam 测试：UIA 成功、失败、空值三种情况下，注键次数为 0、剪贴板写/恢复调用为 0；失败/空值时出现手动复制提示。
- 主动剪贴板翻译与截图 OCR 路径回归仍可用。
- 静态核验：生产代码中不存在可达的自动 Ctrl+C / SendInput 复制回退路径。
- 不操作真实用户剪贴板、不发送真实系统按键做无隔离实验；真实 Win32 GUI 验证如未执行写 NOT RUN。
- 实现者不自审关闭；独立 reviewer 读 diff 后由负责人裁决。

2026-10-22：按用户批准决策转为“移除自动回退”实施，派发置 claimed。尚无本轮实施或执行证据。

## Answer
按用户批准取消自动 Ctrl+C 回退；UIA 失败提示手动复制，生产路径无自动注键或旧剪贴板恢复，主动剪贴板翻译和截图保留。独立 review PASS，负责人批准 resolved。上方 PARTIAL/claimed 均为已被取代的历史，不是现行结论。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 关闭获批范围；不以 PID/sequence 推断绝对归因，不声称真实 Win32 GUI 验证通过。
