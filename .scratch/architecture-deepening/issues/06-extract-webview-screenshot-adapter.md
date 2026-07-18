# 06 — 迁移 WebView adapter 并收缩截图 module

**What to build:** 把非 Windows WebView 选区实现迁移为 ScreenshotCapture seam 的第二个真实 adapter，使共享截图 module 只负责契约、session、归一化和平台 adapter 选择。

**Blocked by:** 05 — 建立 ScreenshotCapture seam 并迁移 Windows adapter

**Status:** resolved

- [x] 非 Windows 选区继续支持 ready 等待、完成、取消、超时和主窗口恢复行为。
- [x] Windows 与 WebView 路径通过同一个 ScreenshotCapture interface 返回一致的 CaptureResult 语义。
- [x] 共享截图 module 不再包含任一平台的选区 UI implementation。
- [x] 平台 adapter 的选择对命令、快捷键、OCR 工作流和前端保持透明。
- [x] 旧平台函数和临时转发路径在所有调用方迁移后被删除，不保留并行 capture 路径。
- [x] 截图按钮、截图快捷键、取消选择和连续请求场景通过相应测试或可用平台 smoke verification。
