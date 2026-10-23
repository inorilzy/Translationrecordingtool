# 15 主窗口恢复可见后重新校验跨窗记录副本

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: architecture / frontend state owner
Source: ARCH-01

## 根因与证据
每WebView独立Pinia，popup收藏直接invoke，仅更新本窗口；`lib.rs:505-509`无变更通知。`HistoryPage.vue:19-21`、`FavoritesPage.vue:12-18`、`DetailPage.vue:12-15`仅mount载入；`App.vue:64-75`仅OCR/导航监听。triager独立重读各入口，无focus重新验证。快捷键成功持久化和popup收藏后，已挂载main仍旧值；路由重进可恢复，非DB丢失。

## 严重性与反证
P2：跨窗同记录展示不一致，局部UI正确性；不是所有数据未保存。实际原生双窗口尚未执行，静态调用链证据高置信但不冒充E2E。

## 范围 / 涉及文件
最小实现选择主窗口重新获得焦点/可见时重新查询当前显示页面，不要求后台实时同步。`src/App.vue`、`src/stores/translation.ts`、`HistoryPage.vue`、`FavoritesPage.vue`、`DetailPage.vue`。为当前页面本地副本建立可注销刷新接口/共享revision，不只刷新store.history。避免旧刷新响应覆盖新路由/新查询。优先这一窄方案；若改为Rust变更事件，需要负责人另明确 `lib.rs`/workflow发布时机和范围，不自发引入通用事件总线。

## 验收与回归
挂载不变，模拟后台新增/enrichment/收藏/取消收藏后main恢复焦点，当前History/Favorites/Detail显示数据库最新状态；卸载注销监听，重复focus不泄漏；查询失败显示错误而非误报空收藏。真实隔离双窗口按architecture.md步骤验收；失败的写操作不产生虚构新记录。

## 非目标
不重构全部状态为单store、不要求窗口在后台实时动画、不重复16的mutation失败契约。

## Comments
2026-10-22：采纳合并history/favorites/detail三个同根因副本问题；与16/17共享文件，建议后做。

2026-10-22 architecture 执行中（任务 #14）：
- 新增窄页面 hook `src/lib/main-window-revalidation.ts`：mount读取，仅main窗口 native onFocusChanged(true)重新查询；页面卸载使请求序号失效，注销监听；异步注册晚于卸载时立即注销。无通用事件总线，无Rust变更事件。
- History/Favorites/Detail接入当前页面刷新；Detail监听路由ID变化并丢弃旧ID查询结果。读操作经可选isCurrent检查后才提交内容/错误；loadHistory保留无参启动调用契约，并以自身序号防止启动查询迟到覆盖。成功读不清共享error，避免抹除无关翻译错误。
- 收藏读取失败返回null，不替换已有列表或显示虚假的空收藏；初次读取失败也只显示错误、不宣称空列表。16已有mutation成功/失败行为保留；Detail await后额外检查record身份避免更换路由时修改新记录。
- 新 `main-window-revalidation.mount.spec.ts` 使用真实store与合成invoke/native-window adapter，覆盖三页重新聚焦、main/popup区分、重复焦点无重复订阅、旧响应/旧ID丢弃、卸载/迟到注册注销、收藏读取失败保持列表与初次失败提示；调整已有收藏spec的null契约及两页mount测试native mock。
- 本会话连续两次Bash工具在命令执行前exit5：Git bash `fatal error - add_item (...Git..., /, ...) failed, errno 1`。这不是测试断言失败；已请现有tests成员在其会话执行专项与build。tests首轮结果：相关4个mount spec 29 passed，settings+translation 52 passed；但vue-tsc报本票新spec第149行TS2345（vi mock不可赋给无参注销函数），Vite build未运行。首次用 `registration.resolve(() => cleanup())` 收窄，负责人指出来源是 deferred 泛型/vi mock 推断；随后彻底移除该处 deferred，改为显式 `new Promise<() => void>` 捕获 releaseRegistration；tests复跑spec 12 passed但vue-tsc仍报line150；最终按负责人建议将 cleanup 声明收窄为 `ReturnType<typeof vi.fn<() => void>>` 并传 `releaseRegistration(cleanup)`，Promise架构及其它测试未动。tests隔离shell复跑确认：该spec 12 passed；`npm run build` exit0（vue-tsc无错，Vite 4600模块）；`npm test` 18文件181 passed，TS2345消失。

限制：以上为jsdom真实store+合成native adapter，不覆盖真实Tauri main窗口焦点、双窗口和真实DB故障。执行阶段曾因Git bash启动失败无法本会话运行，后续以tests及负责人独立执行结果为准。

2026-10-22 triager终审与负责人关闭决策：自动化PASS；已逐读生产hook/三页/store及12个mount用例，确认请求代次、Detail路由watch、卸载/迟到订阅注销、收藏读取失败保留列表。tests的专项12 passed、build exit0，以及负责人在最终工作树执行vue-tsc --noEmit exit0、18文件181 tests passed，作为自动化验收证据。tests报告的E2E 1 passed仅为主翻译mock黄金路径辅助回归，不证明跨窗口焦点行为。**原生onFocusChanged双窗口未验证，属环境限制延期，非已通过**；负责人明确接受为非阻塞残余风险，故Status=resolved。未宣称真实DB故障/native集成通过，未修改源码。

