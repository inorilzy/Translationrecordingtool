# 31 对齐 Vue 家族至修复 SSR 属性名漏洞的补丁

Type: task
Status: resolved
Severity: unassigned (SSR path not demonstrated in this CSR app)
Blocked by: none
Source: 22；../dependency-advisories.md
File lock: package.json/package-lock.json 单一owner串行，与29共享compiler相关锁节点。

## 范围
vue/@vue/server-renderer 3.5.29命中 GHSA-g2v6-rqmx-r4w6；目标Vue及精确内部包一致3.5.42或更高3.5.x，不升3.6预发布。漏洞需SSR输出不可信含CR属性名，本项目createApp CSR没有已证实SSR入口；不是生产P1结论。

## 验收
- Vue/compiler/runtime/server-renderer版本相容，npm ls无invalid/重复不兼容Vue；audit该ID消失。
- vue-tsc --noEmit、Vitest/mount、build、E2E golden path通过。
- 用合成属性名验证上游SSR补丁（隔离测试即可，不给应用新增SSR功能）；确认产物/源码仍CSR。
- 审查lock diff，无无关升级；独立review后裁决。

## Comments
2026-10-22：22取证创建，未实施。开发/未来复用防御性补丁，不声称当前SSR可达。

## Answer
Vue 家族3.5.42 配套及 SSR 兼容 fixture 完成，全部目标副本和真实安装树经独立复核 PASS；负责人批准补丁关闭。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终 181 项/build/E2E 与 smoke](../final-verification.md) 置 resolved；不宣称本应用新增或已证实 SSR 漏洞可达，不替代原生 GUI。此前状态为历史。
