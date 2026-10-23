# 25 模型下载/缓存完整性实现与离线测试

Type: task
Status: resolved
Severity: P2 (05 的实现子票，不另计漏洞)
Blocked by: 24
Owner suggestion: release tooling / PowerShell
Source: SEC-04 model boundary；批准计划“04、24–27：供应链通道”
Historical parent: 05（独立审批门槛已取消，不是当前依赖）
File lock（调度约束）：只写 `scripts/prepare_ocr_models.ps1` 及其测试；不改 24 的 schema/读取器，不改 ORT/词典脚本。

## 当前范围与结论（2026-10-22，默认无 manifest 路径已实现）
- 无 ManifestPath：默认官方固定版本及已获取官方 SHA256，不读取 release-assets.json，不要求用户独立审批；已实现。
- 缓存/目标/下载文件均需基础校验；失败保留旧文件。
- 显式 manifest 保留可选严格模式；依赖24仅用于已完成的严格校验器兼容前置，不是人工审批。
- 新默认路径测试已覆盖无清单默认、不隐式读清单、官方固定标识/摘要、损坏拒绝、失败保旧及显式严格模式：PS5.1 与 PS7 各 21 项通过、均 exit0；独立 review PASS（模型 pins 与缓存事务）。旧18项计数为历史。

## 历史记录（以下范围与关闭证据属于旧强制清单路径）

## 范围 / 涉及文件
`scripts/prepare_ocr_models.ps1`（现状证据见 05：`:43` 仅 inference.onnx 存在即跳过、`:54,60-67` 可变 main 与未校验缓存复制）。
- 通过 24 读取器取批准条目；按不可变 revision 拉取，**全部**必需文件（det/rec onnx、yml 等）逐一校验大小与 SHA-256，不因单一文件存在跳过。
- 缓存与已有目标同样复验；下载写入临时位置，校验通过后才提升到目标；任何失败保留原有效目标。
- 条目缺失/未批准/profile 不受支持时 fail closed（非零退出）。按 21 决策，正式 profile 仅 small。

## 验收与回归（离线 fixture）
假资产 + 假下载器覆盖：正确 hash、错误 hash、缺 yml、截断缓存、身份不匹配、已有错误 target、Force、下载失败、旧有效目标保留、未批准条目拒绝。全部错误非零退出，无静默 skip。不得删除用户真实缓存来让测试通过。

## 与 05 的关系
本票只证明机制正确；05 的“正式资产验证必须 04 完成后执行并记录”仍由 05 承担。04 未完成时本票可 resolved，05 保持阻塞。

## 非目标
不改 OCR 引擎、不测真实推理、不下载真实大模型、不编造摘要。

## Comments
2026-10-22：按批准计划新建；未实施。

## Answer
默认无 manifest 路径模型完整性实现完成：官方固定版本+已获取官方 SHA256 pins、缓存/目标/下载复验与失败保旧；PS5.1/7 各 21 项通过 exit0，独立 review PASS（pins/缓存事务）。显式 manifest 仍为可选严格模式。负责人批准 resolved；未下载真实大模型、未运行真实推理。证据见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：默认路径新测试（PS5.1/7 各21）与独立复核通过后置 resolved。旧 claimed/待验证 及旧18项计数为历史，不覆盖本次结论。04–07 wontfix 不是资产背书；真实模型下载与推理 NOT RUN。

2026-10-22（历史）：旧强制清单路径曾以各18项通过置 resolved；范围取消后重新 claimed，现由上方默认路径结论取代。
