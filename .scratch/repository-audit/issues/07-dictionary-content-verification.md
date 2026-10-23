# 07 词典输入源和缓存按批准清单核验

Type: task
Status: wontfix
Severity: P2
Blocked by: none
Owner suggestion: release tooling / Python
Source: SEC-04 dictionary boundary

## 当前范围变更（2026-10-22）
按用户要求取消独立审批后的正式资产背书/验收门槛，置 wontfix、Blocked by none；不是正式资产验收通过，也不再等待04。基础防损坏收缩至 [27](27-dictionary-integrity-impl.md)：未显式传 manifest 时默认官方固定版本，不读 release-assets.json；结构检查、按 revision 隔离缓存，失败保留旧输入和旧数据库。结构检查不是密码学来源认证。显式 manifest 仅可选严格模式，不要求用户审批。27/28 为 claimed，等待新默认路径测试。

## 历史记录（以下原要求与阻塞已取消，证据保留）

## 根因与证据
`build_dictionary.py:10-29` ECDICT master 可变，download 已有路径即返回；失败下载可能留下部分文件，下次当缓存复用。triager 已重读函数；无已证实污染。

## 严重性与反证
P2：词典导入的可复现性与局部正确性风险；不是SQL注入，导入参数化、WordNet extractfile 不等于路径穿越。

## 范围 / 涉及文件
`scripts/build_dictionary.py` 与04批准清单；ECDICT固定revision，WordNet archive锁摘要；每次复用/下载输入校验，临时文件完整验证后替换。

## 验收与回归
Python临时目录+fake downloader：正确缓存离线复用；错误/截断缓存拒绝；中途网络异常不污染旧输入；hash错误不开始导入；批准数据可生成预期schema及少量测试词条。正式词典资源仍需经批准生成，不能假数据库绕过cargo资源门禁。

## 非目标
不改变单词查询fallback策略（20），不换词典来源或改数据库schema，不下载真实大资产。

## Comments
2026-10-22：等待04；与05/06可在清单稳定后并行，各自只写自身脚本。
2026-10-22（批准计划拆分）：脚本机制实现与离线 fake downloader 测试拆至 [27](27-dictionary-integrity-impl.md)；本票转为正式审批后的最终验收（按 04 条目生成正式词典资源并记录）。新增 Blocked by 27；04 仍为必需前置。上方验收条件全部保留。
