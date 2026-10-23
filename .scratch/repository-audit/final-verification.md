# 第二阶段最终验证与关闭依据

日期：2026-10-22。本轮使用短生命周期subagent/Workflow，无Agent Teams，无commit/push。所有已有脏树改动保留。实施、独立复核、按最终代码及锁文件执行验证分别记录，不把初版绿灯用于后续代码。

## 最终状态与最新真实证据（2026-10-22）

**41票：37 resolved +4 wontfix（04–07），0待办。** 25–28按新默认路径证据关闭；04–07按用户个人工具要求取消强制审批，非验收通过。无manifest默认官方固定来源，显式strict可选。下列证据由用户提供并确认为最终真实证据，本次文档维护未重新实施、测试或构建。

| 最新执行 / 审查 | 结果 |
|---|---|
| 取消人工审批后的Python测试 | 25通过 |
| PowerShell5.1与7模型测试 | 各21通过 |
| PowerShell5.1与7 ORT测试 | 各45通过 |
| 独立默认路径review | PASS |
| 主代理最终串行全树npm测试 / build / E2E | 181通过 / 通过 / 1通过 |
| Node wrapper + dependency smoke | 11通过 |
| Rust check | exit0 |
| Rust全量测试 | 110+4+15=129通过 |
| git diff --check | exit0 |

真实资产下载、真实推理/安装包、GUI/原生信号/跨平台均 NOT RUN；Rust仍14个版本命中公告，详见下方残余，不宣称无漏洞。文档编号/状态/7边DAG/链接机械核验结果见 [map](map.md)。旧Python17、模型18、ORT26、33/4/4及人工blocked全部保留为历史，不能替代本节。

## 历史范围变更与待验证状态（2026-10-22）

按用户最新要求取消独立审批/正式资产背书重门槛：04–07为wontfix、Blocked by none，不是验收通过；不再等待人工签署。24仍resolved，仅作为显式manifest的可选严格校验器。

默认流程要求收缩为：无ManifestPath使用官方固定版本，不读取release-assets.json；模型采用已获取官方SHA256；ORT无hash时大小/唯一ZIP条目/PE版本检查；词典结构检查及按revision缓存；任何失败保留旧文件。显式manifest严格模式仍可选，不要求用户审批。结构/版本校验不是正式资产可信背书。

25–27及28已重新claimed，等待新默认路径测试与复核。**未收到新路径测试，不填PASS、不宣称当前代码已完成。** 现行统计33 resolved/4 wontfix/4 claimed，索引见 [map](map.md)，范围见 [spec](spec.md)。以下旧命令、用例数与失败仅对应旧冻结树，不能沿用为默认路径通过证据；真实模型推理/安装包等未验证边界不变。

## 历史最终执行证据（范围变更前）

环境：Windows x86_64；Node24.14.1、rustc1.96.0；声明Rust最低1.90来自配套依赖MSRV，并未实测1.90。Rust唯一构建执行者使用 `CARGO_TARGET_DIR=C:/translate-audit-target`。

|命令/检查|结果|
|---|---|
|`npm test`（实际Vitest4.1.12）|18文件、181 passed、0 failed，exit0|
|`npm run build`（Vite6.4.5）|vue-tsc及构建exit0；仅既有大chunk提示|
|`npm run test:e2e`|1 passed，exit0|
|`node --test scripts/rust-test.test.mjs`|6 passed，含真实隔离fake cargo CLI exit0/17及非法配置拒绝|
|`node --test scripts/dependency-smoke.test.mjs`|5 passed，含真实Chromium loopback HMR、CSS/map、SSR/cookie、YAML/glob/ZIP、Undici本地HTTP|
|`python -B -m unittest discover -s scripts/tests -v`|17 passed（清单5＋词典12），exit0|
|PowerShell5.1及7的Test-AssetManifest.ps1|各58共享拒绝用例＋加载/选择/内容检查PASS|
|PowerShell5.1及7的Test-PrepareOcrModels.ps1|各18 passed，exit0|
|PowerShell5.1及7的Test-PrepareOrtRuntime.ps1|各26 passed，含最终复验false/throw与回滚失败保留备份，exit0|
|`node scripts/rust-test.mjs check --locked --tests`|最终plist1.10.3锁，exit0|
|`node scripts/rust-test.mjs test --locked`|最终110 lib＋4词典＋15弹窗＝129 passed，0 failed/ignored，exit0|
|`TAURI_CONFIG='{}' cargo check --locked --manifest-path src-tauri/Cargo.toml`|最终仍exit101，明确缺resources/dictionary.db；符合正式资源负向门禁|
|`git diff --check`|exit0，只有CRLF转换提示|
|`npm ls --all --offline`|独立复核exit0，无invalid；目标磁盘版本与lock一致|
|`npm audit --package-lock-only --json`|exit0，vulnerabilities为空；仅当次npm数据库结果|

全部Rust构建/测试已退出；正式发布资源未伪造。原129之前的134/127为历史不同代码计数：取消自动复制回退删除旧测试，新增隐私测试后最终129，并非漏跑。

## 独立复核与修正

- 03：最初复核发现有道任意errorCode和设置serde Display两条日志泄漏，已修为码白名单/unknown及JSON分类行列，补synthetic marker内存捕获；二次review PASS，新树Rust执行覆盖。
- 09：按用户明确审批取消自动Ctrl+C回退，UIA失败给手动复制提示；生产路径无自动注键或旧剪贴板恢复，主动剪贴板翻译与截图保留。独立review PASS。不以序列/PID启发式宣称绝对归因。
- 14：缺Screen cfg导入与AppHandle move后借用已修；正式支持Windows，非Windows保留但NOT RUN。按已批准平台范围关闭，不宣称Linux/macOS编译通过。
- 20/21：词典fail-fast与small迁移规则独立review PASS，生产seam测试通过；不新增自动网络降级。
- 23：真实CLI子进程测试补齐退出码；Windows真实POSIX信号传播未实测，模拟返回路径测试不当成原生实测。负责人接受此明确的平台测试边界，其他验收已满足。
- 24–27：schema严格校验两语言一致；生产synthetic开关缺口已删除。ORT全部复验异常统一回滚，回滚失败保留旧备份；二次review PASS。模型跨角色rename不是断电/多进程原子事务，不作该保证；正式内容信任另由04–07验收。
- 29–35：独立review核对所有目标副本、真实安装树、兼容fixture和HMR后PASS，限定补丁及兼容回归，不代表逐漏洞动态利用复现。
- 36–41：最终独立review PASS，官方依赖约束/MSRV与lock一致，目标公告消失；最终全量129及check已通过。真实GUI/origin隔离、窗口操作和HTTP2端到端NOT RUN；依赖补丁关闭不冒充完整原生产品验收。

## 依赖版本与残余

npm目标：vite6.4.5、postcss8.5.31、nanoid3.3.20、source-map-js1.2.2、Vitest/UI/mocker4.1.12、fflate0.8.3、Vue家族3.5.42、yaml2.9.1、brace-expansion2.1.7、js-cookie3.0.8、undici7.30.1。

Rust目标：tauri2.12.5、runtime2.12.2/runtime-wry2.12.3、utils2.10.2/build/macros/codegen2.7.4；rustls0.23.46、webpki0.103.16、h2 0.4.16、aws-lc-rs1.18.2/sys0.46.0、anyhow1.0.103、crossbeam-epoch0.9.20、serde_with/macros3.21.0；plist1.10.3/quick-xml0.42.0，indexmap2.14.2/hashbrown0.17.1。额外兼容追踪quinn-proto0.11.18、webpki-root-certs1.0.10、rand0.8.6/0.9.3、event-listener5.4.2。

最终独立OSV查询771个registry包版本，仍有14个版本命中；不能声称Rust全图无公告：
- Windows：rand0.7.3的旧phf/parser构建链（RUSTSEC-2026-0097，需特定logger重入，未证明项目可触发）；fxhash/paste/ttf-parser维护公告（无修复版）。
- Windows树外：glib0.18.5、memmap2 0.7.1/0.8.0、quick-xml0.28.2/0.30.0/0.39.2，以及proc-macro-error、zbus_names、zvariant及derive维护公告。旧图形/GTK/Wayland链需跨不兼容边界及对应平台验证，不强制override或删除平台代码来清零。
- Windows plist链quick-xml0.38.4已由41修复。所有36–41指定修复目标无公告命中。原始逐ID证据见dependency-advisories.md；其中旧“XML全为非Windows”的判断已经过本轮纠正。

## 过程中的失败（未隐藏）

1. PowerShell5.1最初无法解析无BOM中文模块，补UTF-8 BOM后5.1/7两宿主通过。
2. npm lock更新后普通install留下旧磁盘模块；发现实际Vitest仍4.1.4，随后npm ci重装并逐模块对比。旧树并行编译期间有2项5000ms超时，不作为最终结果；实际新版串行181通过，未放宽测试超时。
3. Tauri2.11.1被solver配到2.12运行时导致上游编译错误，改成官方匹配2.12.5后通过；未修改上游源码掩盖错误。
4. aws-lc-rs1.17尝试被rustls ^1.18约束拒绝，保持solver选定1.18.2/sys0.46.0，没有降级。
5. HMR临时项目首次被Vite Windows短路径allow检查拒绝，测试改成真实路径且fs.allow仅该临时目录，随后通过；未放宽正式项目服务边界。
6. Git Bash反复add_item exit5，未执行的命令记NOT RUN，恢复后获取明确exit证据。未清用户缓存/终止未知进程。

## 历史人工门禁（已取消，不是当前待办或验收通过）

下述“仍缺/仍被阻塞/现在”只指旧强制清单阶段。当前已取消独立审批及正式资产背书要求，默认路径待25–28新测试；保留原文供追溯。

04仍缺可信独立核验/实际审批人以及ORT zip/DLL与词典的批准摘要。已记录官方固定模型revision、LFS摘要及小配置候选摘要，但同渠道元数据和本机计算不是独立批准。

因此05/06/07正式资产最终验收仍被04阻塞。25/26/27只证明校验机制正确；没有生产release-assets.json，没有下载大模型/ORT/词典，没有真实安装包或模型推理验收。准备脚本现在缺清单会明确拒绝执行，README已说明该有意行为。
