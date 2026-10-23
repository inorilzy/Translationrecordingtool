# 04 审核发布资产的不可变来源与可信摘要清单

Type: research
Status: wontfix
Severity: P2
Blocked by: none
Owner suggestion: release/security maintainer
Source: SEC-04 prerequisite

## 当前范围变更（2026-10-22）
用户反馈“我感觉这个没用吧，这就是个人用的工具，有必要做这个吗”，后续明确取消重门槛、恢复脚本默认可用。取消本票的独立审批/正式资产背书要求，置 wontfix、Blocked by none；不是审批完成或验收通过，也不证明资产安全。无需用户补审批。基础防损坏与失败保留旧文件仍由 25–27 实施，28 等待新默认路径测试。

## 历史记录（以下要求与状态已被上方范围变更取代，保留取证）

## 根因与证据
模型使用 resolve/main，ECDICT 用 master；模型/ORT/词典均凭文件存在复用。现在没有可供 verifier 信任的经审查内容清单，不能凭空填 SHA-256。见 `scripts/prepare_ocr_models.ps1:40-88`、`prepare_ort_runtime.ps1:12-40`、`build_dictionary.py:10-29`。

## 严重性与反证
P2 供应链确定性/完整性缺口。未证明污染或恶意上游；HTTPS 与 DLL 版本 URL 不等同预期内容锁定，也不意味着已被劫持。本票是 SEC-04 的共同研究前置，不再另算第四个漏洞。

## 范围 / 涉及文件
维护者审核各支持模型 det/rec 所需文件、ORT Windows x64 包及最终 DLL、ECDICT CSV/WordNet archive 的版本/revision/URL/大小/SHA-256及获取验证来源。清单预期 `scripts/release-assets.json`（实施时可选等效单一文件，但需同步 05–07）；现阶段只记录审批文档，不自动下载大资产。

## 验收与回归
每个实际使用资产有可信来源、不可变版本、64位十六进制摘要、审核人/方法；明确不支持的 override/profile如何拒绝或另审批。独立渠道核验而非从同一可变页面临时取 hash。用假的清单条目设计 schema 验证后交 05/06/07；不得把假 hash 混进正式清单。

## 非目标
不自动升级 ORT/模型、不发布或签名、不下载用户未批准大文件，不给供应链“绝对安全”保证。

## Comments

2026-10-22 公开来源取证（不是审批，未生成生产清单）：
- 官方 Hugging Face API `PaddlePaddle/PP-OCRv6_small_det_onnx` 当前 commit `28fe5895c24fd108c19eb3e8479f4ab385fbfc62`；ONNX LFS size=9880512，sha256=`d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e`。
- 官方 rec commit `b8f84f0b80c529de40b4fbb3544b84fa7233a513`；ONNX LFS size=21159378，sha256=`5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634`。ONNX 正文未下载。
- 仅下载上述固定 commit 的小配置文件到内存计算候选摘要：det json 229307 bytes / `89240f689a4a77aad75ef55a8df0a15c8e1d4980a327d17e58f24bbadde5aeab`；det yml 885 / `193f435274bf9f0b5f71a929bbfbcf148282df7e633b34e7c373e8f44741b516`；rec json 208004 / `f0bf53c853937a917affdd74467472167727f8ab0f0f7bded01c4a16c27e46e6`；rec yml 150579 / `ab078671bb49f06228eadccd34f1bb501e157f7a047095ffb943ba81512c77d1`。这只是同渠道候选，**不是独立信任核验**。
- `gh api repos/microsoft/onnxruntime/releases/tags/v1.20.1`：发布于2024-11-21，win-x64 zip size=65585496，官方资产 `digest=null`。未下载65MB包，zip与DLL可信SHA-256仍待提供。
- `gh api repos/skywind3000/ECDICT/commits/master`：候选固定commit `bc015ed2e24a7abef49fc6dbbb7fe32c1dadaf8b`（2025-03-28）。未下载CSV；WordNet亦未下载或取得独立批准摘要。
- 因仍缺独立核验、ORT/DLL/词典摘要及真实审核人，本票保持 ready-for-human；不得把以上元数据复制为 approved 生产条目后宣称完成。

2026-10-22：需要人类信任决策，阻塞 05、06、07 的正式内容校验。
2026-10-22（批准计划拆分）：可离线实现的机制拆为 24（schema/读取器）与 25/26/27（各脚本校验实现），它们**不依赖**本票，也**不替代**本票。本票仍为 ready-for-human、人工信任阻塞，不由脚本、本机计算 hash 或合成 fixture 推断完成；05–07 最终验收仍需本票。调度约束：本票的资产研究/填写不得与 24 同时修改清单文件。21 决策后正式 OCR 资产仅需 small profile 条目。
