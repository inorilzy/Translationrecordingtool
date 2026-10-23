# 20 明确词典缺失与损坏时的降级策略

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: translation domain maintainer
Source: ARCH-Q1
File lock（调度约束，非 Blocked by）：等 03 完成 `translation_flow.rs`、21 完成 `lib.rs` 改动后再写对应文件。

> 2026-10-22 产品决策（用户审批计划）：**保留缺词典 fail-fast**，不偷偷联网降级；纠正启动时“将使用 Free Dictionary”的错误提示；区分缺资源与正常查无词。原 `research / needs-info / P2 (conditional)`，转 task/ready-for-agent，尚未派发。以下原文保留为历史。

## 根因与证据
`lib.rs:615-616`称缺词典将用Free Dictionary；`local_dictionary.rs:62-76`返回Err，`translation_flow.rs:87-88`立即终止。triager读取调用链及反证测试 `local_lookup_error_stops_before_online_collaborators:565-590`，明确fail-fast已受测试保护。

## 严重性与反证
仅启动日志与实现矛盾确定；缺失时该发网络还是失败未知。确认应降级后才P2功能问题，不把既有fail-fast擅改成向外发用户文本。句子路径不依赖单词本地lookup，不称全翻译瘫痪。

## 待答 / 涉及文件
产品分别定义missing、corrupt、permission denied、miss四种情形。涉及 `lib.rs`、`local_dictionary.rs`、`translation_flow.rs`与现有测试。若保留fail-fast仅修提示；若允许缺失降级，只将missing映射为unavailable/miss，损坏/读错不无差别吞掉。

## 验收与回归
先记录授权决策和隐私/离线期望，再拆准确实现任务；四种情形断言在线collaborator调用次数及错误提示。不得删除反证测试来迁就未经批准的新行为。

## 非目标
不生成真实词典、不与23的checkout资源准备混为一谈，不偷偷扩大在线发送范围。

## Comments
2026-10-22：needs-info；不是已确定应修fallback的票。

## 2026-10-22 本轮范围与验收
- `lib.rs:615–616`：修正启动提示，如实说明缺词典时单词本地查询将失败（fail-fast），不再声称使用 Free Dictionary。
- 保留 `resolve_translation`、`DictionaryGateway` 现有 fail-fast 契约；保留反证测试 `local_lookup_error_stops_before_online_collaborators`。
- 错误提示区分“词典资源缺失/读取错误”与“正常查无词”。
- 回归：FakeDictionary 断言 missing、corrupt、permission denied/读取错误三种情形在线 collaborator 调用次数为 0 且错误可区分；正常 miss 遵循既有流程。
- 不新增在线降级，不扩大用户文本外发范围。

2026-10-22：按用户批准决策转 ready-for-agent；未实施。

## Answer
保留词典 fail-fast、修正缺失提示并区分正常 miss，未新增自动网络降级；生产 seam 回归和独立 review PASS，负责人批准关闭。见 [最终验证](../final-verification.md)，此前状态为历史。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；正式词典资产审批和验收仍在 04/07，不由本票代签。
