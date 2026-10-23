# 21 明确 OCR 单一 small 产品契约与旧配置迁移

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: OCR maintainer（本轮已派发）
Source: ARCH-Q2
File lock（调度约束，非 Blocked by）：21 先于 20 写 `lib.rs`；不重写已关闭 13 的生命周期实现。

> 2026-10-22 产品决策（用户审批计划）：**正式支持 small 单一 profile**；统一现有迁移规则与文档，旧 profile 明确归一为 small；不新增模型支持、不加 profile 选择器。原 `research / needs-info / P2 (proposed)`，转 task 并派发 claimed。以下原文保留为历史。

## 根因与证据
架构文档:114允许tiny/small/medium；`lib.rs:100-109`更新强制small，SettingsPage当前UI固定small；load normalize与native仍保留其他profile。参见architecture.md Q2，triager重读settings/native normalize确认遗留兼容代码。没有当前UI可选medium后被覆写的可达证据。

## 严重性与反证
P2候选契约/迁移维护问题，不是已确认用户选择丢失。不能为代码分支不一致而评P1，也不自动增加profile选择器。

## 待答 / 涉及文件
产品决定是否仅small，以及旧tiny/medium settings应保留、迁移还是明确拒绝。拟涉及 `docs/architecture-translation-workflow.md`、`src-tauri/src/settings.rs`、`lib.rs`、`native_ocr.rs`，只按决定选最小范围。

## 验收与回归
决策记录后文档、load/update与UI一致；若需要迁移，旧别名/空值/未知值表测，保留用户可理解提示。不同profile资产支持也需告知04清单审核者，不自动扩大下载。

## 非目标
不更换OCR模型，不做13状态修复，不以未执行原生推理作质量承诺。

## Comments
2026-10-22：待产品确认，当前不分配实现。

## 2026-10-22 本轮范围与验收
- 涉及 `settings.rs`、`ocr_contracts.rs`、`native_ocr.rs`、必要的 `lib.rs` 与 `docs/architecture-translation-workflow.md`（:114 tiny/small/medium 描述改为单一 small 及迁移说明）。
- 复用 `normalize_product_ocr_runtime` 与 `normalize_model_profile`；统一空值、tiny/lite/medium/accurate/未知值的归一行为（均归一为 small）及说明。
- 回归：设置加载/更新表测覆盖上述输入；必要的 UI 断言确认显示 small。
- 告知 04/24 清单审核者：正式资产仅需 small 条目，其他 profile 不列入批准清单且应被准备脚本拒绝。

2026-10-22：按用户批准决策转 task，派发置 claimed。尚无实施或执行证据。

## Answer
单一 small 产品契约和旧配置归一规则完成，生产 seam 回归与独立 review PASS，负责人批准关闭。见 [最终验证](../final-verification.md)，此前状态为历史；不代表真实模型推理验收。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved，04/05 真实资产门禁保留。
