# translate-tool 项目审查工作流（2026-04-25）

## 结论摘要
当前项目**不是没有测试**，但测试健全性还不够到“可放心发布”的程度。已有 Vitest 单测/契约测试覆盖了部分 store 与 app-shell / popup 运行时约束，但仍缺少对新引入的手动翻译主流程、导航变更、页面联动、真实 UI 行为和性能边界的验证。

## 当前已观察到的状态
- 新增页面：`src/views/TranslatePage.vue`
- 路由默认入口已改为 `/translate`：`src/router/index.ts:11-18`
- 导航栏新增“翻译”：`src/components/NavigationBar.vue:17-22`
- 启动逻辑仍在主窗口从 `/popup` 回退到 `/history`：`src/main.ts:30-32`
- 已有测试：
  - store：`src/stores/translation.spec.ts`、`src/stores/settings.spec.ts`
  - app shell：`src/app-shell.smoke.spec.ts`
  - popup runtime/chrome：`src/views/popup-window-runtime.spec.ts`、`src/views/popup-window-chrome.spec.ts`
  - 主题/样式/token：`src/lib/theme-bootstrap.spec.ts`、`src/styles/style-token-usage.spec.ts`

## P0：发版前必须补齐

### 1. 对齐主窗口默认落点语义
**依赖：无**
- 核对 `src/main.ts:30-32` 的主窗口回退逻辑是否仍应去 `/history`，还是应该改为新默认页 `/translate`
- 验证以下场景：首次启动、刷新、从 popup 返回、直接访问 `/translate`

**检查点**
- 主窗口和 popup 窗口路由语义一致
- 不出现“默认入口是翻译页，但某些路径又跳回历史页”的分裂行为

### 2. 为 TranslatePage 增加测试
**依赖：路由语义确认后**
- 补至少 1 组页面/组件测试，覆盖：
  - 空输入不能触发翻译：`src/views/TranslatePage.vue:11-15`
  - Ctrl/Cmd+Enter 触发翻译：`src/views/TranslatePage.vue:30-38`
  - 加载态、错误态、结果态渲染：`src/views/TranslatePage.vue:40-69`
  - 剪贴板翻译按钮调用 store

**检查点**
- 新页面不是“只有代码、没有回归保护”

### 3. 为导航与默认路由补回归测试
**依赖：1 完成**
- 补一组针对 `src/router/index.ts` 与 `src/components/NavigationBar.vue` 的行为测试
- 覆盖：默认重定向、导航点击、active 态

## P1：质量与可维护性改进

### 4. 统一错误信息处理
- `src/stores/translation.ts:18-108` 多处直接把捕获对象插入字符串
- 建议统一做 `unknown -> message` 归一化，避免出现 `[object Object]`

### 5. 避免日志页大文件一次性加载
- `src/views/LogsPage.vue:31-35, 109-110`
- 当前是整文件读入并直接渲染 `<pre>`
- 建议增加大小限制、截断、分页或按需加载

### 6. 关注历史页搜索扩展性
- `src/views/HistoryPage.vue:10-17`
- 目前对 `store.history` 做全量过滤
- 现阶段数据量小可接受；若历史记录放大，应补节流、分页或后端查询

## P2：测试体系增强

### 7. 补 E2E
- 当前仓库有 Vitest，但未看到 Playwright 体系
- 建议至少覆盖 1 条黄金路径：
  - 打开主窗口 → 手动输入翻译 → 展示结果 → 收藏/历史可见

### 8. 增加性能冒烟
- 重点场景：长文本翻译、日志大文件、历史记录较多时搜索
- 不一定要完整 benchmark，但至少要有人工/自动化 smoke 基线

## 质量判断
- **测试现状**：中等，不算空白，但对新增 TranslatePage 来说仍明显不足
- **性能现状**：暂无致命问题，但 `LogsPage` 与 `HistoryPage` 有扩展风险
- **最需要做的事**：先解决路由默认语义一致性，再补新页面与导航测试

## 推荐执行顺序
1. 确认 `/translate` 是否应成为主窗口唯一默认入口
2. 修正/确认 `src/main.ts` 路由回退逻辑
3. 补 TranslatePage 测试
4. 补导航/默认路由测试
5. 评估 LogsPage / HistoryPage 的性能边界
6. 再考虑 Playwright E2E
