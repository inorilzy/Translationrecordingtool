# 17 手动翻译入口统一 busy 互斥

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: tests / frontend store owner
Source: TEST-01

## 根因与证据
活动 `TranslatePage.vue:111-112` Ctrl/Cmd+Enter调用无busy检查，按钮才disabled；`translation.ts:121-151`文本/剪贴板可并发且任意响应改current、finally清loading。triager独立执行真实store/deferred invoke：new返回时loading=false而old挂起；old返回后current=old。非仅旧HomePage死代码证据。

## 严重性与反证
P2：用户可触发错误结果和重复请求，未证明关键数据严重损失。既有图片operationId仅保护图片链，无法保护文本。backend已完成的历史持久化不是此次UI互斥目标。

## 范围 / 涉及文件
`src/stores/translation.ts`入口统一拒绝busy时新文本/剪贴板请求，与现有图片busy策略一致；`TranslatePage.vue`键盘handler可补UI保护但不能单独依赖它。测试更新现有store spec及页面mount spec。无需引入全应用latest-wins策略；如负责人选择latest-wins，应先明确取消/计费/历史语义再改票。

## 验收与回归
pending期间Ctrl和Meta+Enter多次，仅首请求invoke；文本/剪贴板/图片交叉发起不绕过busy；拒绝的新请求不得清loading或旧结果；成功/失败finally只归属唯一活动任务；任务结束后可新提交。真实store deferred回归 + 页面键盘mount + npm test/build。

## 非目标
不修改native快捷键全局请求策略、不取消已发provider请求、不把本票扩成HTTP队列系统。

## Comments
2026-10-22：采纳并独立动态复核；17→16→15仅建议文件写入排序，无虚假语义依赖。

2026-10-22 architecture 执行记录（任务 #9）：
- 已在 `translation.ts` 的文本/剪贴板入口增加 busy 早退，返回 null 并提示已有任务；拒绝路径不进入 finally，因此不释放当前请求 loading，也不修改当前结果/历史。图片/截图既有策略保持不变。
- `TranslatePage.vue::handleTranslate` 增加 loading 保护，Ctrl/Cmd+Enter 不绕过按钮 busy。
- `translation.spec.ts` 新增四入口交叉 deferred invoke 回归及文本/剪贴板失败后恢复测试；`TranslatePage.mount.spec.ts` 新增 Ctrl/Meta 首次、重复与结束后重新提交回归。
- 专项：`npm test -- src/stores/translation.spec.ts src/views/TranslatePage.mount.spec.ts`，2 files / 44 tests passed。
- 全量：`npm test && npm run build`，15 files / 165 tests passed，vue-tsc + Vite build passed；仅现有大 chunk 提示，无构建错误。
- 未修改原生快捷键、E2E、15/16票实现；未启动应用/代理，未提交或 push。本记录不宣称原生端到端验证。

2026-10-22 tests 独立复核：审阅上述4文件 diff 并运行 `npm run test -- src/stores/translation.spec.ts src/views/TranslatePage.mount.spec.ts`，exit 0、44 passed；确认 guard 位于 try/finally 前，拒绝不会释放 loading 或覆盖结果，原 owner 在成功/失败时释放；票范围内 PASS，无阻断发现。直接调用 store 发生竞争后 busy 提示保留到下一请求清空，沿用图片入口既有语义，不属本票阻断。完成本票约定验收，标记 resolved。
