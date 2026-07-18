# 05 — 建立 ScreenshotCapture seam 并迁移 Windows adapter

**What to build:** 保持截图命令和快捷键的现有结果契约，同时把 Windows 原生选区、捕获和 DPI 处理隐藏在 ScreenshotCapture seam 后的内部 adapter 中。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 截图按钮和截图快捷键继续返回相同的选区信息与 PNG 数据结果。
- [x] Windows 原生选区 UI、屏幕捕获、DPI 换算和平台资源生命周期由一个内部 adapter 拥有。
- [x] 同一时间只允许一个选区 session；取消、过期 session 恢复和最小选区限制保持现有行为。
- [x] 调用方只需要理解 ScreenshotCapture interface，不需要了解 Win32 消息、窗口类或 GDI 资源。
- [x] 可确定的区域归一化与 session 行为通过 seam 上方的测试验证；平台资源行为通过 Windows smoke scenario 验证。
- [x] 本次迁移不改变截图翻译、OCR、弹窗锚点或最新请求优先规则。
