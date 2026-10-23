# 34 修复 test-utils/beautifier 的 js-cookie 传递依赖

Type: task
Status: resolved
Severity: unassigned (dev-only, vulnerable call not established)
Blocked by: none
Source: 22；../dependency-advisories.md
File lock: package-lock.json/package.json 单一owner串行。

## 范围
@vue/test-utils→js-beautify→js-cookie 3.0.5，GHSA-qjx8-664m-686j；OSV首修3.0.7，不能从audit <=3.0.5自行推断3.0.6安全。目标兼容3.x >=3.0.7。应用无cookie库调用，触发需不可信对象进入assign；不是已证实桌面cookie攻击。

## 验收
- 定向更新锁、满足js-beautify约束，npm ls无invalid，audit该ID消失。
- test-utils mount测试/快照与正常格式化通过。
- 只用隔离合成对象验证原型/属性注入回归，不接触真实浏览器cookie。
- 核实beautifier加载路径并记录prod/dev影响；独立review，无audit fix/无无关升级。

## Comments
2026-10-22：22证据后创建，未实施。

## Answer
js-cookie3.0.8 补丁完成，真实安装树、cookie 合成 fixture 和 mount 兼容回归经独立复核 PASS；负责人批准关闭。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；未接触真实浏览器 cookie，不声称桌面攻击可达或逐漏洞动态复现。此前状态为历史。
