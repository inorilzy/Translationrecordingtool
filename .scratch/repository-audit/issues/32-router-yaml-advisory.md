# 32 定向修复 vue-router 共享 YAML 解析依赖

Type: task
Status: resolved
Severity: unassigned (runtime reachability unproven)
Blocked by: none
Source: 22；../dependency-advisories.md
File lock: package-lock.json/package.json 单一owner串行。

## 范围
yaml 2.8.2经vue-router引入，GHSA-48c2-rrv3-qjmp（深层collection栈溢出），目标>=2.8.3的兼容2.x。应用src无YAML解析，prod标记不能证明翻译输入可达；保留父包API不变，不因该票升路由主版本。

## 验收
- 定向锁更新，全部yaml副本与父约束相容；audit该ID消失。
- 路由相关Vitest/mount、vue-tsc、build及导航E2E通过。
- 隔离合成YAML正常解析/深度错误有界；核实实际bundle是否含解析器并更新可达性证据。
- 不audit fix、不无关升级，独立review，环境阻塞明确记录。

## Comments
2026-10-22：22取证后创建，仅建票，未实施。

## Answer
yaml2.9.1 兼容补丁完成，目标副本、真实安装树和 YAML fixture 经独立复核 PASS；负责人批准关闭。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终兼容回归与当次 npm audit](../final-verification.md) 置 resolved；不把依赖标记当作翻译输入可达或逐漏洞动态复现。此前状态为历史。
