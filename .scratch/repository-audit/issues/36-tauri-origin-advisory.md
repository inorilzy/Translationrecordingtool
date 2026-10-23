# 36 修复 Tauri 本地 IPC origin confusion 公告

Type: task
Status: resolved
Severity: unassigned (framework affected; exploit preconditions require verification)
Blocked by: none
Source: 22；../dependency-advisories.md；GHSA-7gmj-67g7-phm9
File lock: Cargo.toml/Cargo.lock及相关Tauri配套依赖由主负责人单一owner串行；Rust仅一个构建执行者。

## 范围
实际tauri 2.10.3命中[2.0.0,2.11.1)，目标2.11.1或兼容较新2.x，核对runtime/macros/utils/plugins配套约束。远程页面可混淆origin调用本地IPC；本地capability配置不是框架漏洞的充分缓解，需验证远程导航/页面前置。不无关修改CSP或扩大权限，不自动升主版本。

## 验收
- 定向更新并审查所有Tauri相关锁变化，无忽略peer/feature/MSRV冲突。
- OSV按新锁查询该ID消失；Windows离线cargo tree显示生产使用修复版。
- 通过批准23入口定向Rust测试/check --tests及主负责人安排全量；不跳过正式资源检查。
- 用隔离本地合成页面验证local IPC仍正常、非允许origin不能调用；无真实用户数据/真实外部服务。GUI缺环境明确NOT RUN。
- Tauri窗口、快捷键、截图popup、opener最小回归；独立review后裁决。

## Comments
2026-10-22：22取证创建，仅建票，未更新Cargo，不宣称已复现或已修复。

## Answer
Tauri2.12.5 及官方配套锁、MSRV 约束经独立 review PASS，目标公告消失，最终 Rust check 与129项通过；负责人批准依赖补丁及兼容回归关闭。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；**真实 origin 隔离、GUI/窗口操作、HTTP2 端到端 NOT RUN**，不冒充完整原生产品验收。此前失败与状态为历史保留。
