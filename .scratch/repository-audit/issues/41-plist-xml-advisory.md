# 41 更新 Windows plist 链的 XML 解析安全补丁

Type: task
Status: resolved
Severity: unassigned（锁命中，实际不可信XML解析可达性未证明）
Blocked by: none
Source: 36更新后的独立锁复核；RUSTSEC-2026-0194、RUSTSEC-2026-0195

## 根因与范围
Tauri新配套tauri-utils无条件引用plist，旧plist 1.8.0使用quick-xml 0.38.4，因此不能再声称所有XML公告只在非Windows。官方crates.io依赖元数据确认plist 1.10.3使用quick-xml ^0.42.0，允许在现有1.x约束内修复，不需要强制override。

只修改Cargo.lock及必要配套锁节点，不改解析功能。与36–40共享锁文件，串行执行。

## 验收
- plist 1.10.3 / quick-xml 0.42.0及必要indexmap/hashbrown配套，Windows依赖树不再使用0.38.4。
- OSV按新锁复查本链两项公告；其他非Windows旧副本如实单列，不称整个Cargo图无公告。
- 新锁Rust全量与check --tests通过；独立复核后关闭。

## 非目标
不强行升级screenshots/libwayshot的跨0.x图形链，不删除非Windows支持代码，不将锁命中直接判P1。

## Comments
2026-10-22 主负责人通过官方crates.io API确认兼容父版本后执行 `cargo update --manifest-path src-tauri/Cargo.toml -p plist --precise 1.10.3` exit0。solver更新plist1.8.0→1.10.3、quick-xml0.38.4→0.42.0、indexmap2.13.0→2.14.2、hashbrown0.16.1→0.17.1；待最终复查与回归。

## Answer
最终 plist1.10.3/quick-xml0.42.0 锁的129项与check通过，指定目标公告消失，独立 review PASS，负责人批准补丁关闭。纠正旧“XML均在Windows树外”判断，不清除非Windows旧副本事实。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与残余说明](../final-verification.md) 置 resolved；真实 origin/GUI/HTTP2 端到端 NOT RUN，Rust 仍14版本命中，不声称全图无公告。此前状态为历史。
