# 37 定向更新 reqwest 的 TLS/HTTP2 共享安全依赖

Type: task
Status: resolved
Severity: unassigned (production network input; per-advisory prerequisites differ)
Blocked by: none
Source: 22；../dependency-advisories.md Rust网络包逐项表
File lock: Cargo.toml/Cargo.lock单一owner串行，Rust仅一个构建执行者。

## 范围
当前reqwest 0.13.2选择rustls 0.23.37、h2 0.4.13、rustls-webpki 0.103.9。兼容线目标至少0.23.46 / 0.4.16 / 0.103.16。每个公告及别名以报告为准，不能用timeout取代内存/证书修复。
aws-lc-sys 0.38.0首修0.39.0跨0.x minor：只通过aws-lc-rs兼容父版本正常解析升级，禁止override强制越界；若无法满足，记录剩余并另行研究，不阻止兼容子集交付也不宣称全清。
可选quinn-proto（目标0.11.18）、非Windows webpki-root-certs（目标1.0.10）不在当前Windows默认树。可在同次网络锁解析中兼容更新，但必须逐target记录，不把lock匹配等同Windows可达。保留reqwest feature/证书验证策略，不关闭TLS验证。

## 验收
- 更新前后cargo tree --locked --offline --target x86_64-pc-windows-msvc -e features对比，TLS backend、HTTP2、默认策略不变。
- 针对新锁重复OSV查询，按每ID记录消除与剩余；AWS X509/CRL实际使用需区别于crypto provider。
- 通过批准23入口定向HTTP/translator测试、check --tests及安排全量；隔离mock服务验证正常/错误/timeout，无真实API key。
- 主负责人串行执行构建，不能因资源缺失假造dictionary/model；独立review。

## Comments
2026-10-22：22证据创建；仅建票，未实施。跨0.x部分是明确停止条件，不授权盲目升级或删除非Windows/HTTP3支持。

## Answer
rustls0.23.46/webpki0.103.16/h2 0.4.16 及正常父约束解析的 aws-lc-rs1.18.2/sys0.46.0 完成；目标公告消失、官方约束及最终129项/check经独立复核 PASS，负责人批准补丁关闭。见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终执行与独立复核](../final-verification.md) 置 resolved；**HTTP2 端到端、真实 origin 隔离及原生 GUI NOT RUN**，不把 mock/编译通过当成原生实测。Rust 全图仍有14版本公告，见22。
