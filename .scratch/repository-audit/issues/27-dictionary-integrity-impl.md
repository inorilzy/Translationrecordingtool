# 27 词典输入下载/缓存完整性实现与离线测试

Type: task
Status: resolved
Severity: P2 (07 的实现子票，不另计漏洞)
Blocked by: 24
Owner suggestion: release tooling / Python
Source: SEC-04 dictionary boundary；批准计划“04、24–27：供应链通道”
Historical parent: 07（独立审批门槛已取消，不是当前依赖）
File lock（调度约束）：只写 `scripts/build_dictionary.py` 及其测试；Python 侧遵循 24 schema，不改 PowerShell 读取器与其他脚本。

## 当前结论 / Answer（2026-10-22）
默认无manifest官方固定来源、结构检查及revision缓存路径已完成，失败保旧，显式strict仍可选；取消人工审批后Python全套25项通过，独立默认路径review PASS。据用户提供的最终真实证据置resolved，见 [最新验证](../final-verification.md)。25为Python全套数量，不冒称全是词典用例。

## Comments（最新）
仅同步既有执行证据，未重跑产品测试；真实词典资产下载及正式安装包 NOT RUN。以下待验证/12或17项/人工blocked为历史，不覆盖本结论。

## 历史默认路径派发范围（2026-10-22）
负责人曾派发默认流程修改并重新 claimed；当时尚未收到新路径测试。
- 未显式传 manifest：默认官方固定版本，不读取 release-assets.json，不要求用户独立审批。
- 默认结构检查、按 revision 隔离缓存；结构检查不是密码学内容背书。下载/导入失败保留旧输入与旧数据库。
- 显式 manifest 保留可选严格模式；依赖24仅用于已完成的严格校验器兼容回归，不是人工审批。
- 待新测试覆盖无清单默认路径、不隐式读取清单、结构损坏、跨revision缓存隔离、失败保旧及显式严格模式；记录实际命令/exit/数量后交28，旧12/17项不能代替。

## 历史记录（以下范围与关闭证据属于旧强制清单路径）

## 范围 / 涉及文件
`scripts/build_dictionary.py`（现状证据见 07：`:10-29` ECDICT master 可变，已有路径即返回，失败下载可能残留部分文件）。
- ECDICT 固定 commit，WordNet archive 锁摘要，均来自批准条目；缓存每次复用前复验。
- 下载写临时文件，校验通过后原子替换；hash 不符不开始导入。
- 构建失败不删除、不覆盖原有有效 dictionary.db。

## 验收与回归（离线 fixture）
Python 临时目录 + fake downloader：正确缓存离线复用；错误/截断缓存拒绝；身份不匹配拒绝；中途网络异常不污染旧输入；hash 错误不开始导入；构建失败旧词典保留；合成小数据可生成预期 schema 与少量测试词条。

## 与 07 / 23 的关系
合成 db 只用于本票测试，不得放入 `src-tauri/resources/` 以绕过 cargo 资源门禁（23）。07 的正式词典资源生成仍需 04 批准。

## 非目标
不改变单词查询 fallback（20），不换词典来源或数据库 schema，不下载真实大资产。

## Comments
2026-10-22：按批准计划新建；未实施。

## Answer
词典输入完整性机制完成，Python 词典 12 项（连同清单共 17 项）通过，独立复核通过；负责人批准实现子票关闭。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；未生成正式词典或伪造打包资源，机制完成不替代 04/07 真实资产审批验收。此前状态为历史。
