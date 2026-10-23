# 38 定向修复共享 anyhow downcast_mut unsoundness

Type: task
Status: resolved
Severity: unassigned (call-path not demonstrated)
Blocked by: none
Source: 22；../dependency-advisories.md；RUSTSEC-2026-0190
File lock: Cargo.lock单一owner串行，Rust仅一个构建执行者。

## 范围
anyhow 1.0.102经Tauri构建/运行及截图、图像链共享。首修1.0.103，同1.x兼容线。src无直接downcast_mut调用不证明依赖无调用；不把所有error处理都视为触发。

## 验收
- 定向锁更新，不升级无关父依赖；cargo tree与OSV确认所有副本修复。
- 通过批准23入口定向Rust错误路径/截图契约测试与check --tests；全量由主负责人调度。
- 合成错误链/downcast测试正常；不读取真实日志。
- 完整锁diff独立review，工具/资源缺失如实blocked。

## Comments
2026-10-22：22取证拆分，未实施。

## Answer
anyhow1.0.103 及目标公告消失经独立 review PASS，最终 Rust129项与check通过；负责人批准补丁及兼容回归关闭。证据见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；真实 origin/GUI/HTTP2 端到端 NOT RUN，非逐漏洞利用复现，非Rust全图无公告。此前状态为历史。
