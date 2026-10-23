# 26 ORT 下载/缓存完整性实现与离线测试

Type: task
Status: resolved
Severity: P2 (06 的实现子票，不另计漏洞)
Blocked by: 24
Owner suggestion: release tooling / PowerShell
Source: SEC-04 ORT boundary；批准计划“04、24–27：供应链通道”
Historical parent: 06（独立审批门槛已取消，不是当前依赖）
File lock（调度约束）：只写 `scripts/prepare_ort_runtime.ps1` 及其测试；不改 24 的 schema/读取器，不改模型/词典脚本。

## 当前结论 / Answer（2026-10-22）
默认无manifest官方固定来源路径已完成，显式strict仍可选；PS5.1及7模型以外的ORT测试各45项通过，独立默认路径review PASS，据用户提供的最终真实证据置resolved。见 [最新验证](../final-verification.md)。

## Comments（最新）
仅同步既有执行证据，未重跑产品测试；真实ORT资产下载、GUI及安装包 NOT RUN。以下待验证/各26项/人工blocked为历史，不覆盖本结论。

## 历史默认路径派发范围（2026-10-22）
负责人曾派发默认流程修改并重新 claimed；当时尚未收到新路径测试。
- 无 ManifestPath：默认官方固定版本，不读取 release-assets.json，不要求用户独立审批。
- 无官方 hash 时采用大小、唯一 ZIP 条目、PE 版本检查；这不等于密码学内容背书。全部失败保留旧文件。
- 显式 manifest 保留可选严格模式；依赖24仅用于已完成的严格校验器兼容回归，不是人工审批。
- 待新测试覆盖无清单默认路径、不隐式读取清单、大小/条目重复或缺失/PE版本不符、失败保旧及显式严格模式；记录实际命令/exit/数量后交28，旧26项不能代替。

## 历史记录（以下范围与关闭证据属于旧强制清单路径）

## 范围 / 涉及文件
`scripts/prepare_ort_runtime.ps1`（现状证据见 06：`:12-15` 已有 DLL 直接 exit0，`:25-40` 下载解压仅验证存在）。
- 通过 24 读取器取批准条目；Version/Arch 必须精确匹配批准条目，未批准 override 明确拒绝。
- 隔离临时目录下载/解压，校验 archive 与最终 DLL 的 SHA-256；已有目标 DLL 同样校验，不能凭存在跳过。
- 任何失败保留原有效目标并非零退出。

## 验收与回归（离线 fixture）
本地假包/假 DLL 覆盖：正确缓存离线、错误现存 DLL、错误 archive hash、解压缺 DLL、最终 DLL 不匹配、截断 archive、下载失败保留有效目标、未批准 Version/Arch override。全部错误非零退出，无静默 skip。

## 与 06 的关系
假文件不能证明生产 DLL 可信；06 的“真实官方包与最终打包文件校验”仍需 04 批准后由 06 记录。

## 非目标
不调整 native_ocr DLL 搜索规则，不升级 ORT，不发布。

## Comments
2026-10-22：按批准计划新建；未实施。

## Answer
ORT 完整性及最终复验异常统一回滚完成，回滚失败保留旧备份；PS5.1/7 各 26 项通过，二次独立 review PASS。负责人批准机制关闭，见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；合成包不代表真实 zip/DLL 可信或安装验收，04/06 仍阻塞。此前状态为历史。
