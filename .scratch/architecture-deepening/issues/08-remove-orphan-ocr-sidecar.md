# 08 — 删除 orphan OCR sidecar 路径

**What to build:** 移除产品运行时未链接的 OCR sidecar implementation，以及经确认只为该死路径服务的构建和文档残留，使产品只有一条真实的原生 ONNX OCR 路径。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 未链接的 sidecar process lifecycle、health、HTTP recognition 和 vendor dependency installation implementation 被删除或移出产品源码。
- [x] 仅为已删除 sidecar 提供价值的脚本、配置、常量和文档引用一并清理；仍被原生构建使用的资产不得删除。
- [x] 生产 module graph 和架构文档不再把 PaddleOCR 或 RapidOCR sidecar 描述为可选 adapter。
- [x] 原生 ONNX 的模型发现、预热、识别和状态查询保持现有行为。
- [x] 当前遗留设置值仍可被加载并归一到原生 runtime；更窄的用户契约由后续 ticket 完成。
- [x] 原生 OCR 构建与一次真实启动/预热 smoke scenario 通过。
