# 10 — 执行完整架构切换验证

**What to build:** 交付一个完成 settings、截图、供应商和 OCR clean cutover 的可发布应用，并通过端到端场景证明架构加深没有改变现有用户工作流。

**Blocked by:** 04 — 删除 legacy settings representations；06 — 迁移 WebView adapter 并收缩截图 module；07 — 将 ProviderGateway 加深为单一 translate interface；09 — 收窄 OCR 设置与状态契约到原生 runtime

**Status:** resolved

- [x] 手动文本、剪贴板、选中文本快捷键、截图按钮和截图快捷键都通过同一 TranslationWorkflow 行为完成翻译。
- [x] Youdao、Microsoft Translator 和 Google 的选择、凭证错误和成功结果按现有契约工作。
- [x] 设置修改立即作用于所有入口，并在应用重启后正确恢复；旧设置文件可升级而不丢失用户数据。
- [x] Windows 截图选择、取消、连续请求及最新请求优先场景通过；可用环境中的 WebView adapter 也完成等价验证。
- [x] 原生 ONNX 模型发现、预热、状态和一次真实 OCR 识别通过。
- [x] 不存在 legacy settings 双路径、orphan sidecar module、供应商专用 gateway interface 或并行 screenshot capture 路径。
- [x] 相关单元/组件/契约测试、Rust 编译、前端类型检查与应用构建通过。
- [x] 最终架构文档和 dependency graph 与实际代码一致；Brooks architecture audit 不重新出现 Critical dependency finding。
