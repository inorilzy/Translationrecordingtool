# 12 设置持久化成功后才发布 canonical 配置

Type: task
Status: resolved
Severity: P2
Blocked by: 11
Owner suggestion: runtime / settings persistence (same owner as 11)
Source: R4

## 根因与证据
`app_state.rs:96-161` 各helper先写共享配置再persist；失败不回滚。`stores/settings.ts:96-117`仅invoke成功才更新前端。runtime实际theme helper注入Err后内存dark；triager重读确认并核对API/tray同类更新，合并为本根因票。

## 严重性与反证
P2：保存失败UI与后台provider/key/tray状态分裂，后续保存可能顺带提交被拒变更；未证明历史数据库损坏。快捷键单独补偿存在，不能反推其他字段安全，也不得随意删补偿。

## 范围 / 涉及文件
`src-tauri/src/app_state.rs`，必要 `lib.rs`快捷键调用适配及现有设置测试。以串行事务构造candidate→持久化→提交内存；保存失败不改变运行时。不能释放锁后在另一个更新完成时回滚覆盖后者；统一并发边界，不只修theme一处。

## 验收与回归
注入失败测试API/theme/tray及全局快捷键协同：内存、磁盘、调用结果、前端保留值一致；成功提交；两个字段并发更新不丢；失败后下一次成功保存不携带被拒字段；快捷键注册/持久化失败补偿现有行为仍通过。直接测生产helper，不抄测试用逻辑。11原子写保证作为前置。

## 非目标
不改UI交互、不变更凭据存储位置；非所有原生调用都要重新设计事务。

## Comments
2026-10-22：采纳；依赖11，避免内存成功承诺建立在可损坏旧文件的底层上。
2026-10-22：已实现。仅 `app_state.rs`（未改 lib.rs）：四个 helper 统一为 `update_and_persist` → `commit_candidate`（读锁复制 candidate → 持久化 → 成功后写回）；全部设置写入（含 `persist_managed_settings` / `save_persisted_settings`）经 `SETTINGS_COMMIT_LOCK` 串行，存储层不再嵌套加锁。失败不改变内存；下一次保存不携带被拒字段。快捷键命令的注册/回滚补偿路径未改。测试：`cargo test app_state::` 6 passed（Windows，测试进程资源覆盖）；覆盖失败不发布、成功与落盘快照一致、未提前发布、并发 last-writer 一致及迁移原测试。未做真实磁盘/UI 故障注入。
