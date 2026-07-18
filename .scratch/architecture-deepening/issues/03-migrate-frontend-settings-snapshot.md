# 03 — 迁移前端设置为单一 snapshot 与 draft

**What to build:** 让设置 store 持有一个完整设置 snapshot，设置页面只维护一个可编辑 draft，使用户加载、修改、保存和重新打开设置时不再经过多份逐字段镜像。

**Blocked by:** 01 — 扩展 canonical settings runtime seam

**Status:** resolved

- [x] 设置 store 加载后公开一个完整且规范化的 settings snapshot。
- [x] 设置页面从 snapshot 创建一个 draft；用户取消或重新加载时可以恢复到最近一次已保存值。
- [x] 保存供应商、凭证、OCR、快捷键、托盘和主题设置后的用户可见行为与当前版本一致。
- [x] 供应商配置状态、OCR 状态提示、快捷键冲突提示及主题即时应用继续正常工作。
- [x] store 与页面不再各自维护一套逐字段权威状态；页面 draft 不会成为第二个持久化来源。
- [x] 前端测试通过设置加载、编辑、保存、失败回滚和重载结果验证行为，不依赖响应式变量的内部数量或名称。
