# 29 更新 Vite/CSS 共享构建依赖的安全补丁

Type: task
Status: resolved
Severity: unassigned (development/build exposure; not project P1)
Blocked by: none
Source: 22；../dependency-advisories.md npm表
File lock: package.json/package-lock.json 单一owner串行，禁止与其他依赖票同时写。

## 范围
当前 vite 6.4.1、postcss 8.5.8、nanoid 3.3.11、source-map-js 1.2.1。按报告逐项GHSA，目标至少6.4.4 / 8.5.23 / 3.3.18 / 1.2.2，保持现有主版本与所有父依赖约束。Vite与Vue compiler共用PostCSS/map，作为一个共享构建图更新单元；不升级Vite7/8，不顺手更新无关依赖。
开发服务请求可触达文件读取、Windows editor/UNC路径；CSS/map输入主要是构建期。未证明生产远程可利用，不把audit high直接定P1。

## 验收
- 先复核当前lock与报告版本；定向更新，审阅完整lock diff，无npm audit fix、无凭据/真实数据。
- npm ls核验无invalid peer；vite、postcss、nanoid、source-map-js每个副本都满足对应公告修复线，重跑audit逐ID比对；不要只看总数。
- vue-tsc --noEmit、Vitest、npm run build、Playwright golden path通过；Windows loopback dev/HMR及Tauri dev配置不回退。
- 用隔离合成CSS/map测试现有构建功能，不对真实路径做泄漏实验；记录未覆盖的安全触发条件。
- 独立review后由主负责人裁决，资源/工具阻塞如实记录。

## Comments
2026-10-22：22取证后创建；未实施，未改依赖。修复版本来自OSV公开公告，当前ranges允许同主版本方案，但回归未验证。

## Answer
Vite6.4.5/PostCSS8.5.31/nanoid3.3.20/source-map-js1.2.2 已核对全部目标副本和真实安装树，HMR/CSS/map 兼容回归及独立复核通过。负责人批准补丁关闭，见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行证据](../final-verification.md) 置 resolved；当次 npm audit 为空，非逐漏洞动态利用复现或原生 Tauri GUI 验收。此前状态为历史。
