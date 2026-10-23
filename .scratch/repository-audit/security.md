# 防御性安全审计

基线：`d72a76967f3f79d0e51eae4c049889d32f2e9a42`。范围仅本地仓库及本机依赖源缓存；已读 `context.md`、AGENTS 和 docs/agents 三项规范。没有修改源码、启动子代理/应用、访问外部服务、读取真实 settings/日志/历史/剪贴板/密钥或修改远端。下文 P1 为应尽快修复，P2 为常规修复/加固；未发现有证据支持的 P0。

## 候选结论

| ID | 优先级建议 | 证据等级 | 结论 |
|---|---|---|---|
| SEC-01 | P1 | 源码与锁定依赖调用链确证 | Google 请求失败将 API key 写入日志并返回前端 |
| SEC-02 | P2；若存在不可信 WebView 执行入口则提升 P1 | 合成 Rust 测试确证，IPC 注册静态确证 | 日志读取路径检查可被父目录穿越绕过 |
| SEC-03 | P2 | 源码确证；隐私保留策略需产品确认 | 默认 info 日志持久化完整翻译/选中文本 |
| SEC-04 | P2 | 配置确证；未发现实际上游或产物篡改 | 打包下载的模型/DLL/词典未验证预期内容，部分来源可变 |

### SEC-01 — Google API key 出现在请求错误日志

- **证据**：`src-tauri/src/translator.rs:255-258` 将 key 加入 HTTPS URL query；`:266-275` 对 `.send()` 错误直接 `format!("...{}", e)`，调用 `error!` 并返回相同字符串。`src-tauri/src/logger.rs:19-24` 将其同时输出到磁盘日志和 stdout，默认 info 包含 error。
- **锁定依赖复核**：`src-tauri/Cargo.lock:4355-4358` 锁定 reqwest 0.13.2。本地 `reqwest-0.13.2.crate` 中 `src/async_impl/client.rs:3043,3051,3066` 在超时/请求错误中附上 URL；`src/error.rs:267-268` 的 Display 输出完整 URL；该文件 `:14-16` 明确提醒 query key 需 `without_url()`。
- **可达性**：选择 Google 并配置非空 key 后，正常翻译请求遇到连接失败/超时即可触发；不需要攻击者控制远端响应、不需要 XSS。日志查看页通过 `src/views/LogsPage.vue:35` 展示记录。
- **影响**：本来只应发给翻译提供商的凭据进入支持日志、终端及错误展示链，扩大泄漏面；被分享的日志可能允许未授权 API 使用/费用。没有读取真实 key，也没有声称发生过实际泄漏。
- **最小修复**：在格式化 reqwest 错误前调用 `without_url()`；对前端返回不含 URL/凭据的稳定错误。若采用服务支持的凭据请求头，需要额外确认 API 契约，不能仅迁移 query 后忽视其他日志入口。
- **回归验收**：使用假 key 和本地可控失败请求/模拟错误，断言 error Display、捕获日志、IPC 错误均不包含 fake key、编码后的 fake key 或 `?key=`。
- **严格边界**：不能泛化为所有 JSON 解析错误都泄漏。在锁定的 0.13.2 中 `async_impl/response.rs:269-272` JSON decode 没附 URL；本机另有 0.13.5 源其行为不同，本报告结论以锁定版本的请求失败路径为准。未进行真实网络重现。

### SEC-02 — read_log_file 父目录穿越

- **证据**：`src-tauri/src/logger.rs:57-74` 接收任意 `filename`，`log_dir.join(filename)` 后只做 `starts_with(log_dir)`，随后直接 `fs::read_to_string`。Rust `Path::starts_with` 是路径组件前缀判断，不消除 `..`。未要求单一 basename、`.log` 扩展或来自枚举列表。
- **测试**：Windows rustc 1.96.0，在自动清理的临时目录创建 `logs/` 与同级 `synthetic.txt`，以 `logs.join("../synthetic.txt")` 原样复刻检查。断言 starts_with 为 true 且读取到 `SYNTHETIC_ONLY`，通过。没有访问任何真实用户文件。
- **可达性**：`src-tauri/src/lib.rs:743-769` 注册 command，前端 `LogsPage.vue:35` 调用；常规 UI 传枚举项，因此 UI 点击本身不提供攻击输入。恶意 IPC 调用者/被破坏的本地 WebView 脚本可传穿越路径，读取进程账户有权读取的任意可定位 UTF-8 文件。未发现把远端文本直接转换为脚本的入口，因此不宣称远程未认证文件泄漏。
- **能力边界复核**：仓库 `build.rs:1-3` 未配置 application command ACL，未发现 tracked `src-tauri/permissions`。锁定 tauri-build 2.5.6 `src/acl.rs:403-413` 仅在存在应用 permissions 时加入 app manifest；tauri 2.10.3 `src/webview/mod.rs:1801-1806` 仅对 plugin 命令或存在 app manifest 的应用命令强制 ACL。因此当前窗口 capability 划分不能当作自定义 read_log_file/get_settings 的隔离证明。
- **影响**：突破“只读日志目录”的命令契约，扩大 WebView 受损后的本地文件读取面；并不突破 Windows 账户文件权限，也不是已证明的提权/RCE。
- **最小修复**：仅接受一个正常文件名组件、拒绝绝对/前缀/ParentDir 与分隔符；限制日志扩展名；canonicalize 根目录及目标后验证归属与普通文件属性。若威胁模型包含并发本地目录修改，还需处理 symlink/reparse-point/TOCTOU，不能声称单次 canonicalize 完全解决。可另以 app ACL 或 command window 身份检查缩小调用窗口。
- **验收**：合成目录覆盖 `../`、Windows `..\\`、绝对路径、子目录、错误扩展、允许的当前日志；有条件测试 reparse point。原生窗口 IPC 权限回归需隔离 Tauri 环境，本次未运行。

### SEC-03 — 默认日志记录完整用户文本

- **证据**：`src-tauri/src/translation_flow.rs:80-85` 在判断输入类型后 info 打印完整 text；`shortcut_handler.rs:67-70` info 打印抓取到的选中文本。`logger.rs:11-24` 默认 info、每日轮换、保留最多七个文件并复制到 stdout。`translator.rs:125,349,407-414` 的 debug 还会记录服务响应/音频信息，默认级别不触发但可由环境变量开启。
- **可达性**：普通手动/快捷键翻译即触发，无异常条件。翻译工作流在远端调用前记录输入，失败请求亦可能留痕。
- **影响/限制**：可能包含用户选中的商业文本/个人信息，增加相对于历史数据库的第二份可分享记录；用户数据本就存历史不意味着支持日志可默认包含全文。未确认用户是否同意此隐私保留策略，故 P2，不推断监管违规。
- **最小修复**：默认仅记录 request ID、provider、字符数、结果类型/耗时；文本诊断若确有需求，单独显式 opt-in 并警告保留范围。不要只改为 debug 后视作脱敏，也不要将可字典还原的短文本 hash 当成匿名化。
- **验收**：用唯一 synthetic marker 做正常、失败、快捷键 adapter 测试，检查默认日志中没有 marker；不读取实际用户日志。

### SEC-04 — 发布资产未锁定和核验

- **证据/可达性**：`package.json:10-13` native build 必经资产准备脚本。`scripts/prepare_ocr_models.ps1:43,54,60-67,76-88` 从 HuggingFace `resolve/main` 下载/复用缓存，存在 inference.onnx 就跳过，未核对 hash/commit。`prepare_ort_runtime.ps1:12-15,25-40` 固定默认版本 1.20.1 的 GitHub release DLL，但已有 DLL 就跳过，下载后只检查文件存在。`build_dictionary.py:10-28` 的 ECDICT `master` 与 WordNet 下载也只凭存在复用，不验证内容。
- **影响/限制**：上游分支变化或被污染的下载缓存可使相同提交构建不同内容；若被替换的是 DLL，会作为本地原生代码加载。该结论是缺少供应链内容验证，不是证明 HTTPS 被突破、维护者恶意或实际包被篡改。同账户任意写仓库的攻击者已有更广能力，不将此场景当独立提权。
- **最小修复**：将模型/词典 URL 固定到不可变 revision，在受审查清单内锁定每个资产 SHA-256；每次验证缓存、下载和最终复制文件，失败中止；DLL 同样核对已审核 hash（可附加官方签名校验）。不要信任与可变下载一起获取的可变 hash。
- **验收**：只使用本地假资产测试缓存正确/错误/截断、已有目标与错误 hash 的 fail-closed 行为；发布环境另验证实际产物。不要在审计中自动下载大资产。

## 其他边界：已查事实与未升级为缺陷的观察

1. **CSP/IPC**：`tauri.conf.json:22` 的 CSP 为 null，是纵深防御缺口而非 XSS 证明；生产 Vue 源检索未发现 v-html/innerHTML/eval 输出入口（测试清理 DOM 的 innerHTML 不算）。tauri-utils 2.8.3 `src/acl/mod.rs:358-375` 证明非空 inline capabilities 只选择配置项，不自动叠加 `capabilities/default.json`/`desktop.json`；不要把三个来源权限简单相加。建议随 SEC-02 补显式应用权限与必要 CSP，并回归 popup/screenshot。
2. **凭据持久化**：`settings.rs:25-31,127-136` 以明文 JSON 存 key；`lib.rs:320-323` get_settings 返回完整 record。事实已确认，但本地同账户信任/密钥库要求未定义，未单列为已被利用漏洞。建议 OS credential store 与不返回秘密的 view model，作为产品安全需求讨论；不能把隐藏输入框等同加密。
3. **HTTP/Audio**：Rust 请求使用固定 HTTPS provider host，`http_client.rs:3-10` 有 5s connect/15s total 超时，未见忽略 TLS 校验。`translator.rs:360-362` 接受远端 audio URL，`PopupWindow.vue:139-143` 在用户点击后直接 new Audio；缺少 scheme/host 约束可能允许被污染词典响应引起任意媒体请求。未证明 WebView 对特定 scheme、私网请求或内容的实际行为，不称为服务器 SSRF 或脚本执行。建议 HTTPS/可信媒体 host 策略。
4. **输入大小**：HTTP JSON、IPC base64 和 OCR decode 没见应用级总字节/像素限制；`native_ocr.rs:332-345` 解码路径需结合 image 默认限制与实测评估。没有运行巨量输入，不声称已证明 OOM；保留资源耗尽防御项。
5. **DLL 路径**：`native_ocr.rs:201-249` 信任 ORT_DYLIB_PATH，打包资源未找到时搜索 CWD/workspace。默认安装若资源完整先命中可信路径；必须同时证明缺失资源与攻击者能影响搜索目录才是可利用劫持，故本次不把可疑回退单独报为确证 RCE。发布模式建议只允许 bundle 目录并 fail closed，开发回退加 cfg(debug_assertions)。
6. **SQL**：`database.rs:298-366,383-449` 及 `local_dictionary.rs:88-132` 将用户词句/ID 参数绑定。database 的 format SQL 来自固定 REQUIRED_COLUMNS 或布尔选择的固定 access_count 子句，不把它误报为注入。build_dictionary 导入使用 placeholders；WordNet 使用 extractfile (`:152-160`) 而非 extractall，未发现归档路径写穿越。
7. **依赖与发布配置**：Cargo.lock/ package-lock 已存在；本地解析得到 389 个带 resolved 的 npm entry 全有 integrity，域名为 registry.npmjs.org 与 registry.npmmirror.com。没有联网获取 advisory，没有声称依赖无已知 CVE。tests 成员报告 `npm ci --ignore-scripts` 成功、lock 未变，但安装器提示 14 个 vulnerabilities（5 moderate、9 high）；这是安装器汇总而非可达性证据，本次未运行审计/复现，不能列为已确认漏洞，需另行在固定 lockfile 上复核受影响包、运行时/bundler 可达性和修复版本后再定性。tracked `.github` 下无文件，故没有可审 CI action pinning/permissions；远端保护、签名证书、release provenance/发布权限均未检查。配置无签名字段本身不足以证明实际产物未签名。
8. **资源合并**：`tauri.ocr-native.conf.json:3-5` 列表没有 dictionary，基础配置 `tauri.conf.json:70-72` 有 dictionary。最终合并与成品存在性属于发布正确性问题，已交其他成员复核；本报告未实际打包，也不据此断言 DLL 缺失链成立。

## 本次测试与审计限度

- 成功运行 rustc 合成路径测试，stdout：`PASS: lexical guard accepts parent traversal and reads synthetic outside file`，退出 0；临时目录自动清理。首次 shell 对 Rust `!` 转义导致测试编译失败，改用代码占位替换后通过；不是项目编译失败。
- 本地读取锁定 reqwest 0.13.2 / tauri-build 2.5.6 crate 中指定源（tarfile 内存读取，无归档释放），核对 Tauri 2.10.3 / tauri-utils 2.8.3 已缓存源码。没有依赖升级。
- 变体搜索：从 `starts_with(&log_dir)` 扩展到 starts_with/canonicalize/read_to_string，仅发现 logger 接收外来 filename；settings 和 native OCR 的路径由固定目录/profile 构造，未当作同类穿越。逐一检查 provider map_err：Google URL 携 key；Youdao key/sign 在 form，Microsoft key 在 header，未将其同样归为 URL 泄密。默认 info 原文记录有 flow 与 shortcut 两处。建议 CI 加 fake-secret 日志断言与路径恶意组件表测。
- 未运行完整 npm/cargo 基线（由 tests 成员负责），未运行真实 Tauri、IPC 浏览器利用、网络超时请求、恶意音频/大图、安装包签名或 DLL 装载实验。没有执行 npm audit/cargo audit 联网数据库同步；因此 CVE 覆盖与第三方原生内存安全不在已验证范围。

## 交接建议

优先独立复核 SEC-01 的假凭据回归设计和 SEC-02 的路径/权限契约。SEC-01、SEC-02 可各建一个小工单；SEC-03 先确认日志隐私策略；SEC-04 可按共享资产清单设计后细分模型、ORT、词典验证。CSP/app ACL 与密钥库不是修 SEC-01 的前置依赖，避免把紧急脱敏扩成大重构。
