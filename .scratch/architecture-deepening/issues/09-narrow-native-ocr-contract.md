# 09 — 收窄 OCR 设置与状态契约到原生 runtime

**What to build:** 让设置、IPC 和界面只表达当前真实的原生 ONNX OCR 产品概念，同时自动迁移旧 sidecar 设置，避免用户看到 endpoint、sidecar 日志或假 restart 语义。

**Blocked by:** 04 — 删除 legacy settings representations；08 — 删除 orphan OCR sidecar 路径

**Status:** ready-for-agent

- [ ] 用户可配置和查看的 OCR 概念仅包含受支持的模型 profile、预热选项以及原生模型/ORT 状态。
- [ ] 旧设置中的 `paddleocr`、`rapidocr`、endpoint 或其他 sidecar 字段仍能安全加载，并迁移为等价的原生 ONNX 设置。
- [ ] sidecar endpoint、日志路径和仅对外部进程有意义的 restart 控件不再出现在产品 interface 中。
- [ ] OCR warmup、status 和 recognition 返回与原生 runtime 相符的信息及错误，不伪装成外部服务状态。
- [ ] 设置升级不丢失供应商凭证、快捷键、主题、托盘和有效 OCR 模型 profile。
- [ ] Rust 契约测试、前端设置测试和原生 OCR smoke scenario 覆盖旧值迁移、状态显示、预热成功及模型缺失错误。
