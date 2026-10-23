# 18 修正唯一 browser E2E 的结果区域语义断言

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: tests / browser regression
Source: TEST-02

## 根因与证据
`tests/e2e/translate-golden-path.spec.ts:138`期待heading翻译结果；`TranslatePage.vue:153-155`实际region翻译结果、heading结果。triager重读两处确认。tests报告npm run test:e2e退出1，快照已有测试译文，证明不是未翻译。

## 严重性与反证
P2：唯一浏览器回归门禁假红、降低可维护性；不是生产关键翻译失败。纯mock IPC不证明native行为。

## 范围 / 涉及文件
只改 `tests/e2e/translate-golden-path.spec.ts`，断言具有当前可访问性语义的region，并保留实际译文内容断言；删除同文件已失效save_translation mock若确无调用，保证mock契约对应translate_text返回已持久化记录。

## 验收与回归
npm run test:e2e退出0；若译文缺失仍会失败；不得用跳过测试/降低内容断言/改生产标题来让陈旧测试通过。记录Vite mock环境，不报原生全链通过。

## 非目标
不修改生产文案/样式，不引入全新测试框架，不解决23的Rust资源环境。

## Comments
2026-10-22：采纳；独立小票，可与Rust修复并行。
2026-10-22 triager终审及负责人关闭：实际结果region与生产aria-label一致，译文断言限定region内，删除无调用的save_translation mock不改变当前translate_text返回持久化记录契约。tests报告E2E 1 passed，负责人确认采用该执行证据；静态复核PASS，Status=resolved。仅Vite browser mock黄金路径，不代表真实Tauri/数据库/多窗口验收。
