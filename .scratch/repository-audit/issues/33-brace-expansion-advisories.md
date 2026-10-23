# 33 更新 minimatch 共享 brace-expansion 安全版本

Type: task
Status: resolved
Severity: unassigned (build/test glob input)
Blocked by: none
Source: 22；../dependency-advisories.md brace-expansion七项GHSA
File lock: package-lock.json/package.json 单一owner串行。

## 范围
当前brace-expansion 2.0.2，由minimatch共享给vue-tsc/@vue/language-core、glob、editorconfig等。目标>=2.1.7兼容2.x，覆盖GHSA-f886-m6hf-6m8v、3jxr-9vmj-r5cp、mh99-v99m-4gvg、rgw5-rvv9-x895、6j4f-fj2g-mc7p、qhr7-859c-m2p7、q2hr-2g5m-vwhr。不盲升minimatch/glob主版本；不把构建配置模式等同远程翻译输入。

## 验收
- 全部实际副本修复，父依赖semver满足，npm ls无invalid；audit按七ID核对。
- vue-tsc、Vitest、build通过；正常glob行为保持。
- 小型合成异常brace序列在限时子进程中拒绝或有界完成，不消耗主进程内存测试极限。
- 完整lock diff经独立review，未顺手更新无关依赖。

## Comments
2026-10-22：22取证创建，仅建票，未实施。

## Answer
brace-expansion2.1.7 补丁完成，所有目标副本、真实安装树及 glob fixture 经独立复核 PASS；负责人批准关闭。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终兼容回归与当次 npm audit](../final-verification.md) 置 resolved；非逐漏洞动态利用复现，不将构建输入等同远端翻译输入。此前状态为历史。
