# 35 更新 jsdom 的 undici 兼容安全补丁

Type: task
Status: resolved
Severity: unassigned (test-process networking; not Rust translation client)
Blocked by: none
Source: 22；../dependency-advisories.md undici逐项表
File lock: package-lock.json/package.json 单一owner串行。

## 范围
根dev→jsdom→undici 7.25.0，报告列21项GHSA，目标>=7.29.1兼容7.x。覆盖SOCKS/TLS、cookie/cache、retry、WebSocket、解压等独立前置；不宣称本项目全部启用。不升级Node内置undici或Rust reqwest来冒充修复npm包；无需undici8。

## 验收
- 父约束可接受且全部npm副本满足修复线；npm ls无invalid；audit逐ID核对，不只核总数。
- Node24环境下Vitest/mount全量通过；涉及jsdom资源加载使用本地合成HTTP fixture，无真实账号/远端凭据。
- 确认测试默认网络隔离未回退，记录仍未覆盖的SOCKS/cache等路径。
- 完整lock diff独立review；不audit fix、不批量升级。

## Comments
2026-10-22：22取证拆分，未实施。

## Answer
npm undici7.30.1 补丁完成，真实安装树、本地 HTTP fixture 与 jsdom/mount 兼容回归经独立复核 PASS；负责人批准关闭。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；不是 Node 内置或 Rust 客户端替代修复，不声称 SOCKS/cache 等所有触发路径逐项实测。此前状态为历史。
