# 16 收藏失败必须传播给详情页且不更改星标

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: architecture / frontend state owner
Source: ARCH-02

## 根因与证据
`translation.ts:102-118`catch只设error并resolved，`DetailPage.vue:25-30`await后无条件改本地星标，模板不显示store.error。triager独立vm运行真实store并注入拒绝，输出 propagated=false；architecture实际Detail script执行得到DB=0/detail=1。数据库错误即可达，无需跨窗竞争。

## 严重性与反证
P2：向用户虚报收藏成功，非数据库收藏实际丢失；PopupWindow有自身catch且只成功后更新，不是同样根因，不重复扩大票。

## 范围 / 涉及文件
`src/stores/translation.ts`的toggleFavorite明确成功/失败协议；`src/views/DetailPage.vue`仅成功更新且显示错误；检查 `FavoritesPage.vue:30-35` 调用者避免未处理reject。采用抛错或结果联合均可，但一致测试所有实际caller。

## 验收与回归
生产store/组件mount：reject时详情星标不变且错误可见；resolve才更新；取消收藏失败列表条目保留并有提示；没有unhandled rejection；重试成功清旧错误。避免只断言store.error。npm test/build，实际DB失败集成若未做明确说明。

## 非目标
不改Rust/SQLite、不做15跨窗同步、不清理无关UI样式。

## Comments
2026-10-22：采纳并独立动态复核；建议先于15执行。

2026-10-22 architecture 执行（任务 #10）：
- `translation.ts::toggleFavorite` 返回 Promise<boolean>：成功才更新 store 并返回 true，失败保留记录并返回 false；每次尝试清旧错误，无未处理 rejection。
- `DetailPage.vue` 仅成功时更新本地星标，失败展示 role=alert 的 store.error。
- 经负责人补充授权，`FavoritesPage.vue` 仅成功时重新读取列表，失败保留现有条目并展示错误；未做15跨窗口同步。
- `translation.spec.ts` 覆盖两个原始收藏状态的失败保留/成功重试；新增 `DetailPage.mount.spec.ts`、`FavoritesPage.mount.spec.ts` 使用真实 Pinia store 与 invoke mock，验证实际页面星标/条目/提示，非只断言 error。
- `npm test && npm run build`：17 files / 169 tests passed，vue-tsc 与 Vite build passed（仅大 chunk 提示）。初步 store+detail 专项33测试通过。
- 未触及 Rust、E2E；未提交/push/启动应用或代理；真实 DB 故障注入未执行。等待现有 tests 成员独立复核，暂不 resolved。

2026-10-22 tests 独立复核：审阅diff与两份mount spec，运行三份spec exit0、34 passed；确认成功/失败协议、详情星标、收藏列表保留与重试清错符合验收，无阻断发现。非阻断观察：toggleFavorite 开始时清空全局 error，理论上可能与同窗口并发翻译错误共享状态，但不影响本票收藏失败契约，未扩范围修改。票验收已满足，标记 resolved。
