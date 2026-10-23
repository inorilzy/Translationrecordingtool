# 05 模型缓存与目标文件按批准清单核验

Type: task
Status: wontfix
Severity: P2
Blocked by: none
Owner suggestion: release tooling / PowerShell
Source: SEC-04 model boundary

## 当前范围变更（2026-10-22）
按用户要求取消独立审批后的正式资产背书/验收门槛，置 wontfix、Blocked by none；不是正式资产验收通过，也不再等待04。基础内容校验收缩至 [25](25-model-integrity-impl.md)：无 ManifestPath 默认官方固定版本及已获取官方 SHA256，不读 release-assets.json；失败保留旧文件。显式 manifest 仅可选严格模式，不要求用户审批。25/28 为 claimed，等待新默认路径测试，不沿用旧通过结论。

## 历史记录（以下原要求与阻塞已取消，证据保留）

## 根因与证据
`prepare_ocr_models.ps1:43` 仅 inference.onnx 存在就整目录跳过；`:54,60-67` 可变 main 和未校验缓存复制。triager 已读完整脚本；任意截断文件亦能满足存在性，未运行真实下载。

## 严重性与反证
P2：同提交构建不可复现、缓存损坏/替换进入产物；非已证实恶意模型/RCE。与06/07共享供应链主题，但本票仅模型缓存信任边界及自己的验收。

## 范围 / 涉及文件
`scripts/prepare_ocr_models.ps1` 及04批准清单（只读取、不擅改信任值）；实施时添加脚本级测试。按不可变 revision 拉取，验证全部必需文件，不因单一 onnx 存在跳过。临时下载验证通过后才提升到目标；不以失败下载覆盖有效缓存。

## 验收与回归
本地假资产/假下载器测试正确/错误hash、缺 yml、截断 cache、已有错误 target、Force、每个受支持 profile；错误 fail closed，不能通过退出0。有效缓存离线可用，失败保留原有效目标；最终复制内容复核。正式资产验证必须04完成后执行并记录。

## 非目标
不改 OCR 引擎、不测试真实推理、不顺带改 ORT/词典脚本或编造摘要。

## Comments
2026-10-22：实现细节充分，但被04阻塞；ready-for-agent 不代表可越过依赖。
2026-10-22（批准计划拆分）：脚本机制实现与离线 fixture 测试拆至 [25](25-model-integrity-impl.md)；本票转为正式审批后的最终验收（用 04 批准条目对真实资产执行并记录）。新增 Blocked by 25；04 仍为必需前置。上方验收条件全部保留，不因 25 通过而视为满足。
