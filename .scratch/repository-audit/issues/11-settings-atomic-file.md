# 11 设置文件使用完整写入后的原子替换

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: runtime / settings persistence
Source: R5

## 根因与证据
`settings.rs:127-136` fs::write最终settings.json，先截断再写；`:112-124` 损坏JSON报错，`lib.rs:565-569` 启动回默认。triager核对完整链；基于标准库语义，未实际断电/杀进程。

## 严重性与反证
P2：保存时异常中断可损失用户配置；范围是单应用配置，不是数据库广泛毁损，不升级P1。正常fs::write成功不反驳中间失败，序列化发生在写前则序列化失败本身不损坏文件。

## 范围 / 涉及文件
`src-tauri/src/settings.rs`，必要时 `Cargo.toml`/`Cargo.lock` 仅为平台正确原子替换工具。同目录临时文件写完整、适当sync、Windows支持覆盖的原子替换；失败保留最后有效文件并返回错误。不得先remove旧文件再rename。确定并记录对断电/目录sync保证的边界。

## 验收与回归
实际保存函数+临时配置目录：已有有效配置，注入临时写失败/替换失败后旧JSON仍完整；成功后全部新值；首次写入；多次覆盖；临时文件清理与权限。Windows真实原子替换测试；不读真实配置、不填满磁盘。恢复策略不得将损坏文件静默覆盖掉证据。

## 非目标
12才处理内存提交；本票不改provider规则、不迁移密钥库、不把非原子写改名为原子。

## Comments
2026-10-22：采纳，为12建立持久化成功/失败承诺。
2026-10-22：已实现于 `src-tauri/src/settings.rs`（同目录 create_new 临时文件 → 完整写 → sync_all → 关闭句柄 → 标准 `fs::rename`）。Windows 上不存在“先删旧文件”步骤，因此目标被其他进程打开（未共享 delete）时 rename 失败并保留最后有效 JSON；权衡是保存返回错误而非以损坏文件换取写入成功，调用方需按失败处理（内存提交见 12）。普通失败清理临时文件；非正常杀进程可能遗留 `.settings-*.tmp`，不会被加载。不承诺目录项断电持久性。定向 `settings::tests` 22 项通过（Windows；含真实目标锁失败后释放并重试成功）。Unix 权限测试仅编写，本平台未运行。
