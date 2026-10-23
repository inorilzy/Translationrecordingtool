# 39 定向修复图像并行链 crossbeam-epoch 指针格式化

Type: task
Status: resolved
Severity: unassigned (specific formatting path unproven)
Blocked by: none
Source: 22；../dependency-advisories.md；RUSTSEC-2026-0204
File lock: Cargo.lock单一owner串行；Rust仅一个构建执行者。

## 范围
crossbeam-epoch 0.9.18由image/rayon-core→crossbeam-deque共享，Windows prod选入。目标>=0.9.20兼容0.9线，漏洞前置是invalid pointer的fmt::Pointer调用，不是任意图像解析。

## 验收
- 定向锁更新、父约束满足；OSV该ID消失、cargo tree无旧副本。
- 通过23入口运行现有图像/截图/OCR契约定向测试及check --tests，不下载模型或真实图片。
- 如添加回归，仅用安全合成对象，不执行UB型PoC危及主进程。
- 全量由主负责人调度，完整锁diff独立review。

## Comments
2026-10-22：22取证创建，未实施。

## Answer
crossbeam-epoch0.9.20 补丁及目标公告消失经独立 review PASS，最终 Rust129项与check通过；负责人批准补丁及兼容回归关闭。证据见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；真实 origin/GUI/HTTP2 端到端 NOT RUN，不以合成契约测试代表真实 OCR 模型或 UB 利用复现。此前状态为历史。
