# 07 — 将 ProviderGateway 加深为单一 translate interface

**What to build:** 让 TranslationWorkflow 通过一个 translate interface 使用远程翻译，使供应商选择、凭证校验和 vendor adapter dispatch 集中在一个深 module 中。

**Blocked by:** None — can start immediately

**Status:** resolved

- [x] workflow 和翻译解析策略只调用一个 ProviderGateway translate 操作，不再选择供应商专用方法。
- [x] Youdao、Microsoft Translator 和 Google 的成功结果、凭证要求和用户可见错误保持现有行为。
- [x] 单词本地词典命中、Free Dictionary 补全、本地 miss 后远程回退和错误优先级保持不变。
- [x] 供应商 identity 和不同凭证形状不再泄漏到 gateway 调用方。
- [x] 测试 fake 只需实现一个 translate interface，即可覆盖三种供应商选择、缺失凭证、远程失败和取消场景。
- [x] 不引入插件框架、通用依赖注入容器或并行供应商策略路径。
