# 06 ONNX Runtime 下载包和已有 DLL 均需核验

Type: task
Status: wontfix
Severity: P2
Blocked by: none
Owner suggestion: release tooling / PowerShell
Source: SEC-04 ORT boundary

## 当前范围变更（2026-10-22）
按用户要求取消独立审批后的正式资产背书/验收门槛，置 wontfix、Blocked by none；不是正式资产验收通过，也不再等待04。基础防损坏收缩至 [26](26-ort-integrity-impl.md)：无 ManifestPath 默认官方固定版本，不读 release-assets.json；无 hash 时校验大小、唯一 ZIP 条目与 PE 版本，失败保留旧文件。结构/版本检查不是密码学来源认证。显式 manifest 仅可选严格模式，不要求用户审批。26/28 为 claimed，等待新默认路径测试。

## 历史记录（以下原要求与阻塞已取消，证据保留）

## 根因与证据
`prepare_ort_runtime.ps1:12-15` 已有DLL直接 exit0，`:25-40` 下载解压仅验证文件存在。triager 重读确认；固定 version URL 未验证内容，旧版本 DLL 也可在指定新 Version 时被复用。

## 严重性与反证
P2：不一致或不可信原生库会进产物；DLL为执行代码使完整性重要，但尚无污染/搜索劫持利用证据，不报P1/RCE。

## 范围 / 涉及文件
`scripts/prepare_ort_runtime.ps1`，读取04批准清单；隔离临时下载/解压后校验archive和最终DLL，已有目标同样核验版本/摘要。未批准 Version/Arch 明确拒绝。

## 验收与回归
使用本地假包/假DLL测试正确缓存离线、错误现存DLL、错误archive hash、解压缺DLL、最终DLL不匹配、下载失败保留有效目标、未批准override；全部错误非零退出，无静默skip。真实官方包和最终打包文件校验另记录，不能用假文件证明生产DLL可信。

## 非目标
不调整 native_ocr DLL 搜索规则，不升级ORT、不发布；不修改模型/词典准备逻辑。

## Comments
2026-10-22：等待04批准可信摘要。
2026-10-22（批准计划拆分）：脚本机制实现与离线假包测试拆至 [26](26-ort-integrity-impl.md)；本票转为正式审批后的最终验收（真实官方包与最终打包 DLL 按 04 条目校验并记录）。新增 Blocked by 26；04 仍为必需前置。上方验收条件全部保留。
