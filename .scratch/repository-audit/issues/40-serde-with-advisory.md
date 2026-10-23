# 40 定向更新 Tauri 共享 serde_with 安全版本

Type: task
Status: resolved
Severity: unassigned (specific serializer reachability unproven)
Blocked by: none
Source: 22；../dependency-advisories.md；GHSA-7gcf-g7xr-8hxj
File lock: Cargo.lock/Cargo.toml单一owner串行，与36共享Tauri图；Rust仅一个构建执行者。

## 范围
serde_with 3.17.0经tauri-utils同时用于build/prod。KeyValueMap序列化空sequence/map项可panic，首修3.21.0，同主版本；核验MSRV与serde_with_macros配套，不默认需要升级Tauri主版本。src没有KeyValueMap调用，不把普通空译文当成触发证据。

## 验收
- 正常父依赖解析得到>=3.21.0兼容3.x，MSRV/配套宏兼容；不强制override不兼容父约束。
- cargo tree所有副本修复、OSV该ID消失；仅此共享单元锁变化。
- 使用23入口设置/配置序列化定向测试、check --tests及主负责人安排全量；合成空map/sequence正常或显式错误，不panic。
- 独立review，资源/MSRV冲突如实blocked，不批量升级。

## Comments
2026-10-22：22取证创建，未实施。

## Answer
serde_with/macros3.21.0 配套及目标公告消失经独立 review PASS，最终 Rust129项与check通过；负责人批准依赖补丁及兼容回归关闭。证据见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；MSRV1.90 仅依赖声明、实际rustc1.96.0；真实 origin/GUI/HTTP2 端到端 NOT RUN。此前状态为历史。
