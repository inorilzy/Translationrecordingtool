# 测试基线与前端审计

## 授权修复与工单23研究更新（2026-10-22）

任务#8：工单18已将E2E断言改为region“翻译结果”，保留且收紧译文内容断言至该region；确认生产代码没有save_translation调用后删除过期mock。`npm run test:e2e` exit0，1 passed (3.9s)。未改生产文案或Rust构建配置。

工单23：已锁定tauri-build 2.5.6源码 `src/lib.rs:459,474-478,537-554` 确认支持TAURI_CONFIG环境JSON merge patch，并根据合并后的bundle.resources复制资源。以下仅对子进程覆盖，shell和配置文件保持原状：

```sh
node --input-type=module -e 'import {spawnSync} from "node:child_process";const r=spawnSync("cargo",["test","--locked","--manifest-path","src-tauri/Cargo.toml"],{stdio:"inherit",env:{...process.env,TAURI_CONFIG:JSON.stringify({bundle:{resources:[]}})}});process.exit(r.status??1);'
```

同样可将test替换为check。实测该test命令已越过dictionary.db资源检查，进入真实tauri编译；退出101：`failed to build archive ... libtauri-88afb5c5b140856d.rlib: 磁盘空间不足。(os error 112)`。**Rust测试实际执行数仍为0，不报告通过**。检查剩余空间仅43,245,568字节；为避免继续写满磁盘未运行第二个覆盖check或release构建，也未擅自清理共享target。父进程`Object.hasOwn(process.env,"TAURI_CONFIG")`为false，证明没有持久环境覆盖。

### C盘重跑结果（2026-10-22，用户授权改构建目录）

首条 `CARGO_TARGET_DIR=C:\translate-audit-target` 经shell/JS转义报os123，未编译；改用**正斜杠** `C:/translate-audit-target`（等价目录）后：

| 命令（均通过Node spawnSync为子进程注入 env） | 退出码 | 结果 |
|---|---:|---|
| cargo test --locked --manifest-path src-tauri/Cargo.toml；CARGO_TARGET_DIR=C:/translate-audit-target；TAURI_CONFIG={"bundle":{"resources":[]}} | 0 | 全量编译1m57s；lib 72 passed、local_dictionary_tests 4 passed、popup_window_contract_tests 15 passed；main/doc 0；合计**91 passed，0 failed** |
| cargo check --locked --tests（同上env） | 0 | 1m05s，仅dead_code/unused import警告 |
| cargo check --locked（**无** TAURI_CONFIG覆盖，默认target） | 101 | `resource path resources\dictionary.db doesn't exist`；证明该覆盖只为子进程，正常/发布严格资源检查未放松 |

警告：`ocr_contracts.rs:17,58,72` dead_code；`settings.rs:97,101` load/save_settings_record 在非test构建dead_code；`examples/ocr_profile_bench.rs:8` unused import。不是失败，未另定级。

注意：该C盘target目录可在后续受控验证复用；仅本审计临时构建，非发布资源验收。D盘默认target在当前改动后再次无覆盖check仍以资源缺失失败（与基线相同）。

取舍：可作为纯编译/unit测试的临时命令；该产物不含正式资源，不能运行OCR或用于发布。不得把空resources覆盖设为全局环境或写入正式配置。tauri-build会跟踪TAURI_CONFIG变化，恢复正常命令时资源检查仍在；此前无覆盖test/check已实测失败，覆盖后的正常/release再次验证因磁盘不足尚未执行。资源复制列表为空不绕过其他系统依赖、编译、link或运行期错误。本研究没有伪造资源、修改build.rs/Cargo配置、下载大型资产或启动原生应用。

---

基线 d72a769；2026-10-22；Windows win32/x64，Node 24.14.1，npm 11.13.0，cargo/rustc 1.96.0，Rust target x86_64-pc-windows-msvc。结论 **PARTIAL**：前端 unit/typecheck/build 通过，browser E2E 有确定的断言漂移，Rust 被 checkout 资源缺失阻塞，不能声称 native 或全门禁通过。

## 执行安全边界

已读 context.md、项目域文档约定、package.json/Cargo.toml、Vitest/Playwright 配置、verify-s06.mjs 和相关测试。原生入口会触及真实配置/快捷键，未运行 tauri dev/build 或应用；未执行模型/词典/DLL 下载脚本，未安装系统组件，未发布。Playwright 仅启动回环 Vite 与 headless 浏览器，使用合成 Tauri IPC，不访问实际用户历史/设置。无源码/锁文件修改，无子代理。git status 最后仅显示 `.scratch/repository-audit/` 未跟踪，构建/test-results 为忽略产物。

## 命令与退出码

| 命令 | 退出码 | 结果 |
|---|---:|---|
| node --version / npm --version / cargo --version / rustc --version | 0 | 上述版本 |
| npm run test（安装前） | 1 | vitest 不存在；初始 node_modules 缺失，不是源码缺陷 |
| npm run build（安装前） | 1 | vue-tsc 不存在，同上 |
| npm ci --ignore-scripts | 0 | 从原锁安装304 packages；禁用生命周期脚本；锁未改变 |
| npm run test（安装后） | 0 | 15文件、157 tests passed，3.22s |
| npm run build（安装后） | 0 | vue-tsc --noEmit 与 Vite 6.4.1 production build 通过；JS 1547.77kB / gzip427.90kB，只有chunk>500kB警告 |
| cargo test --locked --manifest-path src-tauri/Cargo.toml | 101 | build-script-build exit1: resource path resources\\dictionary.db doesn't exist；没有执行到Rust测试 |
| cargo check --locked --manifest-path src-tauri/Cargo.toml | 101 | 同一资源缺失阻塞 |
| npm run test:e2e | 1 | 1 test failed：heading locator 漂移，见 TEST-02 |
| Node + TypeScript transpile + vm 执行真实 store 并注入 deferred invoke | 0 | 确证 TEST-01；前两次实验stub不完整分别因exports/require未定义退出1，补全模块mock后成功 |
| Node 读取 package-lock engines / git diff --stat | 0 | Node兼容性矛盾见TEST-03；无tracked diff |

没有 lint script，不假装执行 lint。verify:final 已审阅，它只顺序重复 unit/Rust test/check/build，在已知同一资源失败处停止，因此未额外运行。未执行打包/真实OCR/native UI/全局快捷键测试。Rust资源是 README:222-233 已说明的准备前置条件；此 checkout 缺失不直接定为产品缺陷，不伪造正式资源让门禁通过。依赖安装提示14 vulnerabilities(5 moderate/9 high)已交 security，安装器汇总不证明产品可利用性。

## 确证候选缺陷

### TEST-01 / P2：键盘提交绕过 busy 限制，旧翻译覆盖新翻译

- 位置：`src/views/TranslatePage.vue:2-8,67-69,111-112,123`；`src/stores/translation.ts:138-151`（clipboard路径121-135同样无入口guard）。HomePage.vue:19,37 为相关旧页面入口，不依赖其是否可路由作为主要证据。
- 按钮通过 disabled 阻止重复请求，但 textarea Ctrl/Cmd+Enter 无 loading 检查；submitTranslation 只检查 trim 空白。store 无请求ID或 busy guard，任何返回都覆盖 currentTranslation，任何 finally 都设 loading=false。
- 复现：在 /translate 输入A，Ctrl+Enter启动慢请求；仍在处理中改为B再Ctrl+Enter。B先返回、A后返回时，画面最终回到A；B返回时 loading已false，即使A仍在进行。
- 无native的动态证据：读取真实 translation.ts，用 TypeScript transpile 在vm执行，mock defineStore、真实Vue ref、deferred invoke及history helper。调用 translateText('old')、translateText('new')，先resolve第二个，再第一个，输出：`after new: loading=false, current=new, old pending=true`，然后 `after old: current=old`。不是仅根据代码猜测。
- 最小修复范围：store入口统一互斥（与现有图片busy策略一致），或明确latest-wins请求token并让loading归属活动请求；TranslatePage键盘路径测试和store逆序返回测试。不要只disable按钮。
- 严重性理由：用户可触发的错误结果/重复请求，不是P0/P1；不声称已证明跨所有native入口一致latest-wins需求。

动态复现核心（Node ESM，未落盘测试文件）：
```js
const mocks = {
  pinia: { defineStore: (_, fn) => fn }, vue: { ref },
  '@tauri-apps/api/core': { invoke: () => new Promise(r => pending.push(r)) },
  './translation-records': { mergeTranslationIntoHistory: (h,p) => [p,...h], updateHistoryFavoriteState: () => [] }
};
const c = { exports: {}, require: n => mocks[n] };
vm.createContext(c);
vm.runInContext(ts.transpile(fs.readFileSync('src/stores/translation.ts','utf8')), c);
const store = c.exports.useTranslationStore();
const a = store.translateText('old'), b = store.translateText('new');
pending[1]({ id:2, source_text:'new' }); await b;
// loading false while a pending
pending[0]({ id:1, source_text:'old' }); await a;
// currentTranslation.source_text === 'old'
```

### TEST-02 / P2：唯一 browser E2E 的 heading 断言与UI语义漂移

- 位置：`tests/e2e/translate-golden-path.spec.ts:138`；`src/views/TranslatePage.vue:153-155`。
- npm run test:e2e 确证失败，expect(getByRole('heading',{name:'翻译结果'}))超时。实际 `翻译结果` 是 section aria-label（region），heading 为 `结果`。
- Playwright生成快照 `test-results/translate-golden-path-main-93356-lates-text-and-shows-result/error-context.md:72-78` 已显示region翻译结果、heading结果以及测试译文。因此这是测试漂移，不是翻译未渲染。
- 最小修复：对region翻译结果或当前heading结果断言，保留译文内容断言；复跑唯一E2E。
- 该suite完全mock IPC，不能代替native回归；`save_translation` mock:88-95仍代表旧契约，而现store由translate_text直接返回已持久化record。清理mock契约可随该测试修复，不另开大型重构。

### TEST-03 / P2：文档 Node18+ 与已锁定依赖最低版本不兼容

- 位置：`README.md:211`、`package.json:33-46`（无engines）、`package-lock.json:3550-3580`（jsdom29.0.2）。
- 锁文件直接核对：vitest4.1.4 node `^20.0.0 || ^22.0.0 || >=24.0.0`；jsdom29.0.2 node `^20.19.0 || ^22.13.0 || >=24.0.0`；naive-ui2.44.1 node `>=20`。Node18不在支持集合。
- 证据为发布的依赖engine contract，不声称在本机执行Node18失败（本机24）。复现核验：读取锁中各包engines并与README对照；严格engine-check的Node18安装将不满足声明。
- 最小修复：明确受支持LTS基线（例如当前已验证Node24），README、package engines及CI同步；不要升级锁以掩盖问题。

## 覆盖缺口与交接

- 15个Vitest文件混合store单测、mount和静态源码断言；157绿不代表原生行为通过。缺失Ctrl+Enter busy/乱序返回测试，恰好漏TEST-01。
- 设置store会把invoke错误保存并抛出；底层内存/磁盘失败一致性由runtime主报，未重复定级。收藏失败吞错/详情错误显示由architecture ARCH-02主报；跨窗口缓存由architecture主报。
- 非Windows screenshot编译路径缺import/值移动由runtime主报；未装目标或系统库，Windows检查也尚未越过资源构建脚本，不能声称编译已验证。
- native OCR模型、DLL、真实截图选择、系统剪贴板恢复、全局快捷键、发布安装包完整性均未测试。resources overlay合并交security，不以本次frontend build证明安装资源齐全。
- 无P0/P1前端确证候选；Rust blocked不等于passed。
