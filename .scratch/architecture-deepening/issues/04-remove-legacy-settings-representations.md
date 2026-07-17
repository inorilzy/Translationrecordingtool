# 04 — 删除 legacy settings representations

**What to build:** 完成 settings expand–contract 切换，删除已无调用方的完整设置镜像、手工字段映射和并行默认值路径，使 Rust canonical record 成为唯一设置权威。

**Blocked by:** 02 — 迁移后端设置调用方到 canonical record；03 — 迁移前端设置为单一 snapshot 与 draft

**Status:** ready-for-agent

- [ ] 已迁移调用方不再通过 legacy settings record、兼容 alias 或字段到字段 mapper 工作。
- [ ] Rust 中只有一个完整设置记录拥有默认值、持久化形状和运行时配置数据。
- [ ] 前端所需 fallback 由跨语言契约保护，不再手工维护一套可能漂移的完整默认值。
- [ ] 当前设置 IPC 形状和现有设置文件在本 ticket 完成时仍向后兼容；OCR 产品契约收窄留给后续 ticket。
- [ ] 删除旧路径后不存在双写、双读或“新旧任一可用”的降级逻辑。
- [ ] Rust 与前端设置测试、类型检查和应用构建均通过。
