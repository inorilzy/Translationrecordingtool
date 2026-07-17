# 01 — 扩展 canonical settings runtime seam

**What to build:** 在不改变现有用户行为的前提下，引入一个由 Rust 拥有的 canonical settings runtime seam，统一设置默认值、加载、保存、快照与序列化；旧设置文件和现有调用方在迁移期间继续工作。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] 现有设置文件（包括缺少新增字段的旧文件）加载后得到与当前版本相同的有效设置。
- [x] canonical settings record 是 Rust 默认值的唯一新增权威，并继续序列化为现有 camelCase IPC 形状。
- [x] 设置经过加载与保存 round-trip 后，供应商凭证、OCR 配置、快捷键、托盘、主题及预热选项保持不变。
- [x] 当前生产调用方可在 expand 阶段继续使用兼容形式，用户可观察行为不发生变化。
- [x] 设置 seam 的测试覆盖默认值、round-trip、缺失字段补全和旧值归一化，而不检查私有 helper 或源码布局。
