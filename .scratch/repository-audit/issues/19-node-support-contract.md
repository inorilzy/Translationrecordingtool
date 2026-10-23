# 19 对齐 Node 支持说明与锁定依赖 engines

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: tests / developer tooling
Source: TEST-03

## 根因与证据
README:211承诺Node18+，package.json无engines；triager独立读取lock：vitest4.1.4支持20/22/24+，jsdom29.0.2要求20.19/22.13/24+，naive-ui>=20。Node18确定不在集合；本机仅Node24.14.1已执行，不伪称Node18运行失败。

## 严重性与反证
P2：开发入门承诺与实际工具不兼容，非应用用户运行时漏洞。安装器可能只warn不等于依赖支持Node18。

## 范围 / 涉及文件
`README.md`开发环境与 `package.json` engines，必要时npm自动同步 `package-lock.json`根metadata（不得升级依赖）。最小保守基线为已验证Node24 LTS（24.14.1及以上24.x）；若保留20/22须核对完整锁定依赖集合并实测相应最低版本，不凭单个package推断。现仓库无tracked CI，不为此凭空引入发布流水线。

## 验收与回归
文档/engines明确一致；选定版本npm ci --ignore-scripts、test、build通过；不支持版本engine严格检查可报告不兼容；lock包版本/integrity不变。若范围调整须在Comments解释依据及真实测试版本。

## 非目标
不升级依赖、不解决22的advisory，不声称Node18一定启动崩溃。

## Comments
2026-10-22：采纳文档/工具契约问题，P2不是安全漏洞计数。
2026-10-22 tests 实施：README 改为列出锁定依赖 engines 交集（20≥20.19.0 / 22≥22.13.0 / 24+，21/23 被 vitest4 排除，仅 24.14.1 实测）；package.json 新增 `engines.node: ">=24.0.0"`，package-lock.json 仅在根 "" 条目增加同值 engines，未改版本/resolved/integrity。独立发现：lock 根 version 仍为 0.2.0 而 package.json 为 0.2.3，未在本票顺带修改。验证（隔离 shell，exit0）：`npx vitest run src/views/main-window-revalidation.mount.spec.ts` 12 passed；`npm run build`（vue-tsc+vite）通过；`npm test` 18 文件 181 passed；`npm ls --depth=0` exit0。注意：`>=24.0.0` 与 README 交集说明略宽于“仅 24 实测”，本机 `npm ci --ignore-scripts` 在 Node 24.14.1 已实跑（此票修改前，依赖未变）。
2026-10-22 tests 按 triager 文案修正：README 改为明确“项目要求 Node ≥24.0.0（推荐 24 LTS，仅 24.14.1 实测）；engine-strict 会拒绝 20/22；部分上游依赖仍声明 20/22，但不是本项目支持版本”，删除“全 lock 交集”表述以免超出已核传递依赖范围。package.json/lock engines 保持 `>=24.0.0`。修改仅 README 文本，未改包版本/依赖。此前同一类型检查/build（vue-tsc、Vite build）exit0 与 181 tests 通过仍有效（此文案不影响代码/依赖）。2026-10-22 triager重读README:211、package.json engines与lock根engines确认均为项目Node>=24.0.0；20/22明确非支持，24.14.1为唯一实测。依赖版本未升级，既有lock根版本差异不在本票修复。按负责人授权及前端构建/181 tests证据，终审PASS，Status=resolved；未声称实测所有>=24版本。
