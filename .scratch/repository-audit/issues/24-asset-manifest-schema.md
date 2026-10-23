# 24 发布资产清单 schema 与共用校验基础设施

Type: task
Status: resolved
Severity: P2 (SEC-04 实现子票，不另计漏洞)
Blocked by: none
Owner suggestion: release tooling（PowerShell 共用读取器单一 owner；Python 遵循同一 schema）
Source: SEC-04；批准计划 `vectorized-scribbling-puppy.md`“04、24–27：供应链通道”
Historical parent: 04（独立审批要求已取消；本票仅保留可选严格校验器）
File lock（调度约束，非 Blocked by）：04 资产研究不得与本票同时修改清单文件；清单 schema/读取器由本票单 owner 写，25–27 只读调用。

## 当前定位（2026-10-22）
仍为 resolved，仅代表已有可选严格 manifest 校验器的旧实现与测试证据。默认准备路径不再读取 release-assets.json，不要求用户审批；原独立审批/正式背书要求已取消（04–07 wontfix），不是被本票代签通过。显式传 manifest 才选择严格校验；不为日常个人使用新增人工门槛。25–27 重新 claimed 验证默认路径及严格模式兼容，本票旧通过结论不覆盖新增行为。

## 历史记录（以下为旧强制清单范围、调度约束与执行证据）

## 背景与拆分口径
原 04 是人工信任审批（不可变来源、真实可信 SHA-256、审核人/方法），05–07 是正式内容校验验收。本轮按计划把“可离线实现与测试的机制”拆为 24–27 实现子票，原 04–07 的验收条件**不变、不弱化**：24–27 即使合成测试全部通过，也不代表 04 已批准或 05–07 已关闭。

## 范围 / 涉及文件
- 定义小而明确的 manifest schema（建议 `scripts/release-assets.json` 或等效单一文件，最终路径实施时记录并同步 25–27）：每条目至少含资产类别（model/ort/dictionary）、标识（profile/文件名、版本、架构）、不可变来源 URL/revision、大小、64 位小写十六进制 SHA-256、审批状态与审核人/方法字段。
- 实现严格加载/校验：未知字段、缺字段、hash 格式错误、重复条目、未批准（非 approved）条目、未列出的 profile/version/arch 均拒绝，返回非零/抛错，不做静默 skip。
- PowerShell 共用读取器（供 25/26 调用）与 Python 读取逻辑（供 27）遵循同一 schema；两侧行为表测一致。
- 合成 fixture 只放在测试目录，明确标注为 synthetic。

## 验收与回归
- 离线表测：合法 fixture 通过；上述每类非法输入均失败且错误可读；Python 与 PowerShell 对同一 fixture 结论一致。
- **生产清单中不得出现合成/本机计算 hash**；如生产清单文件在 04 批准前创建，只能为空集或全部条目处于未批准状态，从而使准备流程 fail closed。
- 记录实际执行命令、exit code 与用例数；未执行的不写 PASS。

## 非目标
不填写任何正式 hash，不下载大资产，不修改三个准备脚本的下载逻辑（25–27），不代替 04 的人工信任决策。

## Comments
2026-10-22：按批准计划新建，派发后置 claimed。尚无实施或执行证据。

## Answer
严格 schema 两语言校验一致，生产 synthetic 开关缺口删除；Python 清单 5 项、PS5.1/7 各 58 共享拒绝用例及内容检查通过，独立复核通过。负责人批准机制关闭，见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；无生产 release-assets.json，不代表 04 审批或 05–07 正式资产验收。此前状态为历史。
