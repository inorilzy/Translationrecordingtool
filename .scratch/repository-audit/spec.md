# Repository audit：证据复核与小粒度修复规格

## 最终状态（2026-10-22）

**41票：37 resolved（01–03、08–41）+4 wontfix（04–07），0待办。** 25–28已有默认路径新证据及独立review PASS；04–07按用户个人工具要求取消强制审批，不是验收通过。默认无manifest使用官方固定来源，显式strict可选。

用户提供的最终真实证据：Python25通过；PS5.1/7模型各21、ORT各45通过；独立默认路径review PASS；主代理最终串行全树npm181/build/E2E1通过、Node wrapper+dependency smoke11通过、Rust check exit0与110+4+15=129通过、git diff --check exit0。见 [最终验证](final-verification.md)。真实资产下载/GUI/原生信号/跨平台 NOT RUN，Rust14版本公告残余保留，不宣称无漏洞。本次只同步文档、未重跑产品测试。

7条依赖保持11→12、24→25/26/27、25/26/27→28；最终文档机械校验见 [map](map.md)。下方33/4/4及人工blocked、旧测试和旧当前标题均为历史。

## 历史范围与中间状态（2026-10-22，取消重门槛）

用户反馈“我感觉这个没用吧，这就是个人用的工具，有必要做这个吗”，后续明确要求取消重门槛、脚本默认可用、04–07取消或收缩。此节优先于下方全部旧范围/状态。

| 状态 | 数量 | 编号 |
|---|---:|---|
| resolved | 33 | 01–03、08–24、29–41 |
| wontfix | 4 | 04–07 |
| claimed | 4 | 25–28 |
| 其他 | 0 | 无 |
| 总数 | 41 | 01–41连续 |

- 04–07取消独立审批/正式资产背书要求，Blocked by均none；不是审批或验收通过，不要求用户补审批。基础防损坏仍由25–27承担。
- 无ManifestPath默认官方固定版本，不读取release-assets.json；模型用已获取官方SHA256；ORT无hash时大小/唯一ZIP条目/PE版本检查；词典默认结构检查+按revision缓存；全部失败保留旧文件。结构/版本校验不等于密码学来源认证。
- 显式manifest仅可选严格模式，24仍resolved，定位为可选严格校验器；它不是默认流程或人工审批前置。
- 负责人已派发25–27，本次只改文档；25–28重新claimed，等待新默认路径测试与独立复核，不宣称当前代码已实现或新测试通过。28当前Blocked by为25/26/27；25–27保留24作为已完成的严格模式兼容前置。
- 当前7条边、逐票索引及拓扑见 [map](map.md)。旧37 resolved+人工blocked为历史；[final-verification](final-verification.md)旧测试仅对应旧冻结树，不能验证新默认路径。真实推理/安装包及原生未测边界继续保留，不把取消门槛写成这些项目通过。
- 只写本审计目录文档，不改源码/README、不派代理。本次仅校验文档状态、链接和DAG，实际校验结果单列map；没有运行新产品测试。

### 历史中间状态文档校验
Python只读机械校验exit0：41票连续唯一，33/4/4统计与map逐行一致；7条依赖有效无环且当前Mermaid一致；123个本地Markdown链接目标有效。仅文档验证，不代表新默认路径代码/测试通过。

## 历史快照（以下全文旧范围、状态与测试均非现行裁决）

日期：2026-10-22  
基线：`d72a76967f3f79d0e51eae4c049889d32f2e9a42`  
交付状态：负责人已完成本轮实现、独立复核及最终验证，本次仅同步工单/map/spec；不修改源码/依赖/README，不派代理、不 commit。最终执行证据以 [final-verification.md](final-verification.md) 为准，非本次文档维护者重跑。

## 历史状态（2026-10-22 旧人工门禁裁决）

| 状态 | 数量 | 编号 |
|---|---:|---|
| resolved | 37 | 01–41 除04/05/06/07 |
| ready-for-human | 1 | 04 |
| ready-for-agent | 3 | 05、06、07（均被04阻塞） |
| claimed / needs-info / needs-triage / wontfix | 0 | 无 |
| 总数 | 41 | 01–41连续 |

本轮新增关闭25票：03/09/14/20/21–41；原12张关闭票保持原证据不重复补无关结论。41票索引、34条依赖及拓扑层见 [当前map](map.md)。28已将22及29–41纳入依赖；28不依赖04–07，其关闭不代表人工资产门禁通过。

### 关闭范围与不能代签的残余
- 03隐私日志、20词典fail-fast与21单一small契约实现及独立复核通过。09按用户批准取消自动Ctrl+C回退，UIA失败提示手动复制；旧PARTIAL为历史而非当前结论，不以PID/sequence启发式宣称绝对归因。
- 14仅按Windows正式支持范围及静态修复关闭；Linux/macOS target NOT RUN，非跨平台成功。
- 23真实CLI退出码回归完成；**Windows真实POSIX信号传播 NOT RUN**，模拟返回路径非原生信号实测。
- 24–27机制通过，不能替代04可信独立审批或05/06/07真实模型/ORT/词典验收。05 Blocked by 04,25；06 Blocked by 04,26；07 Blocked by 04,27。尚无生产release-assets.json、真实模型推理、正式资产/安装包验收。
- 29–41关闭的是指定补丁及已批准兼容回归；**36–41真实origin隔离、GUI/窗口操作、HTTP2端到端 NOT RUN**。不冒称完整原生产品验证。

### 22公告核实完成；Rust剩余14版本公告（非全部修复）
最终OSV查询771个registry包版本，仍14个版本命中。Windows旧rand0.7.3构建链及fxhash/paste/ttf-parser维护公告、Windows树外glib/memmap2/旧quick-xml及其他维护公告保留；Windows plist旧XML由41修复。详情见 [最终残余](final-verification.md) 与 [逐ID历史报告](dependency-advisories.md)。npm当次audit为空不能推广为Rust全图无公告或全仓安全保证。

### 权威执行摘要
最终Vitest18文件181通过、build/E2E通过、Rust最终129通过且check通过；wrapper6、依赖smoke5、Python17通过，PS5.1/7清单/模型/ORT分别58拒绝用例及内容检查/18/26通过；正式资源负向门禁仍exit101。具体命令、exit、独立复核、历史失败及测试边界均见 [最终验证](final-verification.md)。旧134/127等不替代最终129。

### 本次文档校验
Python只读机械校验exit0：41票连续唯一、34边存在且无环、状态37/1/3、当前map索引与票面一致、工单/map/spec共116个本地链接目标有效、本轮25张Answer及证据链接存在。首次Bash启动exit5，后实际执行恢复；仅正则转义SyntaxWarning，不冒称未执行的产品测试通过。

## 历史记录（以下旧状态、失败与边界保留，非现行结论）

以下所有“当前/本轮/现行”均指记录当时；旧分诊五值、23/28票统计和09 PARTIAL不覆盖上方最终裁决。

### 第一阶段实施状态快照（2026-10-22）

以下为负责人后续授权实施与验收后的现行状态；其余章节的“本轮”、测试基线及五值Status规则保留为初始审计历史，不代表当前修复树。后续负责人明确授权使用`resolved`关闭已验收票；triager仅更新文档，不修改源码。

| 状态 | 初始审计 | 当前 |
|---|---:|---:|
| resolved | 0 | 12 |
| ready-for-agent | 16 | 4 |
| needs-info | 5 | 5 |
| ready-for-human | 1 | 1 |
| needs-triage | 1 | 1 |
| 总数 | 23 | 23 |

已关闭01、02、08、10、11、12、13、15、16、17、18、19；05、06、07、09保持ready-for-agent，05–07仍依赖04。08/13本次由负责人批准关闭，证据为稳定树clipboard 10项、shortcut_handler 7项、native_ocr生命周期6项、ocr_contracts 6项通过，全量126 passed/0 failed及cargo check --tests exit0（非triager重跑）。08不构成完整剪贴板保护：延迟渲染阻塞、部分Set失败不可回滚、真实Win32未测；09正在实施，未验收关闭。sequence/owner/PID匹配不能证明唯一归因本次Ctrl+C，同目标进程主动/延迟写入仍需独立评估，不默认降低09验收要求。13保留Ready发布短窗口、failure仅按profile、progress全局、锁中毒恢复未验证、真实模型/DLL/UI未测，代码冻结不清理。各票及[map](map.md)记录详细边界；未执行的原生验证不记为通过。

### 09最终补强证据与未关闭范围

09整体仍为**PARTIAL / ready-for-agent**，不拆票降低原验收。已实施并静态复核通过：快照在同一OpenClipboard guard内记录稳定非0 sequence；注键前过期校验失败时零键、零恢复；应用整个fallback串行化；恢复比较与Empty/Set同OS guard；无token失败不恢复；注键错误传播并始终尝试Release。第二轮冻结树由tests串行执行：clipboard 14 passed、shortcut_handler 11 passed、cargo check --tests exit0，全量115 lib+4 dict+15 popup=134 passed，0 failed/ignored/skipped，四条命令均exit0。第一轮131是旧冻结树证据，不替代本轮。

仍未解决：同目标进程主动/延迟写入与本次Ctrl+C无法可靠区分；快照检查至注键存在非原子窗口，原票“用户后写始终胜出”未全面满足。新增channel协调的真实A/B线程只验证生产串行化seam争用和顺序，数据/按键为合成路径；**真实Win32临界区、序列及剪贴板往返均未验证**。保持既有延迟渲染阻塞、部分Set失败不可回滚等边界。撤回的Original/Modified授权及snapshot覆盖方案未实施。

本次逐票Status/Severity复核确认：01为唯一已确认P1且已resolved；无已确认P0或其他未关闭的已确认P1。统计仍12/4/5/1/1，总23；此结论仅限已审计工单，不能宣称全仓无漏洞。

## 第二阶段：批准计划下的剩余工单（2026-10-22）

用户已明确审批 `C:/Users/zhiyu_liu/.claude/plans/vectorized-scribbling-puppy.md`。其行为决策写入各票票头，原文保留为历史：
- 03：info/debug 均不记原文/译文/完整响应/敏感 URL，保留类型/字符数/耗时/请求ID/状态，不加敏感日志开关。
- 09：取消自动 Ctrl+C 回退；UIA 失败/空值提示手动复制后用剪贴板翻译。这是用户批准的功能范围变更（需在用户文档记录兼容性变化），不是 owner/PID/sequence 启发式修补；原“用户后写胜出”目标以移除自动注键满足。
- 14：仅 Windows 为正式已验证平台；修明确 cfg 编译错误，非 Windows 无法执行即标未验证。
- 20：保留缺词典 fail-fast，修正“将使用 Free Dictionary”提示，区分缺资源与正常 miss。
- 21：正式仅 small，旧 profile 归一 small，不新增模型。
- 23：采用测试专用 Node wrapper（`TAURI_CONFIG` 覆盖 `bundle.resources=[]`），不改正式配置。

供应链拆分：原 04 为人工信任审批、05–07 为正式资产最终验收，**验收不变**。可离线实现的机制拆为 24（schema/读取器）→ 25/26/27（模型/ORT/词典脚本），05–07 分别新增 Blocked by 25/26/27 并保留 04。24–27 合成测试通过不得替代 04 或关闭 05–07；生产清单不得写入合成/本机计算 hash。28 为本轮集成验收门禁。22 证据到位后才从 29 起建最小修复票。

Status 口径：本阶段按 issue-tracker 规范使用 `claimed`（已派发实施中）与 `resolved`（经独立复核验收）；其余仍用五值分诊标签。上方“分诊规则”中“不用 claimed/resolved”的说法为初始审计历史。现行 DAG、文件调度锁与统计见 [map](map.md)。

## 目标与边界

复核 [context](context.md) 及 [architecture](architecture.md)、[security](security.md)、[runtime](runtime.md)、[tests](tests.md) 四份报告；沿实际源码检查可达性、反证和测试证据，合并同根因，产出 `issues/NN-slug.md` 与 [map](map.md)。不是穷尽证明，不宣称全仓无漏洞。

用户要求只创建文档，本轮只新增 `.scratch/repository-audit/` 文档。没有读取真实settings/历史/日志/密钥/剪贴板，没有真实provider请求、原生应用启动、远端写入、发布、依赖升级。既有测试成员安装依赖及生成忽略产物的范围见tests报告，不能误称整个团队没有执行测试。

## 分诊规则

- P0：广泛灾难性损失或关键可利用问题；本次无达到证据门槛的候选。
- P1：关键流程严重失败或严重数据风险；01 Google请求错误泄漏可用于计费API的凭据，正常网络故障可达，属于严重敏感数据风险，列P1；未证明真实外传/费用损失。
- P2：局部正确性、边界加固、可维护性；不因文件长、风格、模块多、依赖安装器high标签直接提P0/P1。
- 未验证/需求未决不装作确证缺陷；其 proposed/conditional Severity 不参与确证统计。
- Status 严格使用 `needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`（按 docs/agents/triage-labels.md）。本轮不用 wayfinder 的 claimed/resolved，以免与分诊值冲突；实现完成以后追加 `## Comments` 验收记录，由维护者管理完成信息，不伪造新Status。
- `Blocked by: NN` 只表示必要技术/信任前置；同文件排队是所有权协调，不额外制造伪依赖。`ready-for-agent` 且Blocked by未解除仍不可执行。

## 复核方法与证据等级

triager 在四依赖任务完成后开始；已读所有报告、context、issue-tracker/triage-labels/domain规范，并独立重读核心函数、前端实际活动页面、脚本与相关文档。

1. **独立动态复核（不写测试文件）**：Node加载真实 `src/stores/translation.ts`，TypeScript transpile+vm+deferred invoke。两个请求逆序返回：new完成而old pending时loading=false；最终current=old。收藏invoke拒绝未向caller抛出。对应17、16；不是原生IPC或DOM测试。
2. **独立锁定依赖核对**：内存读取本地 `reqwest-0.13.2.crate` 的 `src/error.rs:267-268`，Display包含完整URL；结合生产translator send map_err和security核实的client附URL路径，确认01。未用本机另一个版本替代锁定版本。
3. **独立契约核对**：读取package-lock engines：vitest20/22/24+、jsdom20.19/22.13/24+、naive-ui>=20，README18+矛盾成立（19），没有运行Node18。
4. **源码+已有实验审查**：逐项重读剪贴板/guard/settings/status、日志目录检查、跨窗缓存、准备脚本。runtime合成Rust原函数执行与security路径测试作为已提供证据，但triager未重复执行native、未把抽取harness当完整生产集成。
5. **反证**：词典fail-fast专门测试、popup收藏失败有catch、普通LogsPage没有用户任意路径入口、Windows cfg隐藏非Windows编译错误、Node安装warning不等于实际崩溃，均纳入票据。

## 确证结论与去重口径

13个确证行为/契约缺陷：01、02、08、09、10、11、12、13、15、16、17、18、19（1个P1、12个P2）。另一个共同的供应链内容验证缺口SEC-04，按模型/ORT/词典三个独立实施边界拆为05–07，04是可信清单研究前置；不要把04–07统计成4个新漏洞。03隐私政策、14平台支持、20词典降级、21profile迁移尚待产品确认；22仅advisory线索；23为环境阻塞/测试可维护性研究。

- 合并：settings API/theme/tray失败一致性只建12；跨窗history/favorites/detail只建15；Google请求错误多个输出sink只建01；模型/DLL/词典共用04信任源，不重复创置信清单。
- 不合并：08格式丢失与09所有权覆盖可独立成立；11文件中断损坏与12Err后内存改变不同故障模式；16失败虚报与15跨窗陈旧不同根因；18测试漂移不是17生产并发问题。

## 本轮测试基线（来源 tests.md，非 triager 重跑）

| 项 | 结果与限度 |
|---|---|
| npm test | 15文件/157 tests passed，安装依赖后退出0 |
| npm build | vue-tsc+Vite通过；chunk警告不是故障 |
| browser E2E | 1失败，已有译文但heading断言漂移，18处理 |
| cargo test/check --locked | 均exit101，build script缺resources/dictionary.db，未进入Rust测试；23跟踪 |
| verify:final | 未重复运行，已知将停在同一Rust资源前置；不能报绿 |
| native/OCR/安装包/跨平台 | 未执行；不能用frontend/mock测试替代 |

## 实现验收原则

每票必须包含生产入口/涉及文件、单一根因、严重性依据、复现/反证、最小范围、回归和非目标。修复优先在真实函数/组件seam新增可失败回归，不复制整份源码构造“永远只验证副本”的替代测试。

01需要真实生产错误formatter假凭据回归，并区分完整IPC/日志集成是否已跑；05–07不编造hash，正式资产由04批准。08/09/10/13/15的native验收条件写在对应票据，不因无法启动而勾选通过。11/12磁盘测试使用临时目录与失败注入，不动真实配置。

只对本票最小修复验收；不把CSP、密钥库、通用事件总线、平台移植、整个crate拆分塞进局部票。研究票未定需求前不自动派实现。

## 实施交接

23张独立票的编号/状态/DAG与推荐批次详见map。负责人应先理解证据与约束，再向现有成员明确派发文件范围；本审计没有授权开始这些修复。

本轮完成标准：文档完整、同根因去重、状态有效、编号连续、依赖存在且无环、明确未覆盖与环境阻塞。这个标准仅表示审计交付完成，不表示票据完成或产品安全保证。
