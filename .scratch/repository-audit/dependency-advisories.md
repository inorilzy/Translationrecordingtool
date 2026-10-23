# 22 依赖公告取证（2026-10-22）

## 范围、快照与结论

- 本文只研究，不改依赖、不安装工具、不执行 audit fix。批准计划覆盖原22票“非目标”中的联网限制；仅发送包名/版本等依赖元数据，没有读取用户凭据、翻译历史或真实日志。
- 采样开始 `2026-10-22T14:45:03Z`；Node `v24.14.1`，npm `11.13.0`，registry `https://registry.npmjs.org/`。锁文件 SHA-256（真实计算，不是资产 hash）：`package-lock.json` = `f38a85f41154eb8e02fb34cc021ae66c74741be99f12b98cbae5866e95fdfaac`；`src-tauri/Cargo.lock` = `12a4c92c5a4c3c32d0afd90bad6a272de4c4019d1e92d8a89bd27582d2644c56`。
- `npm audit --json --package-lock-only` 成功返回审计结果，exit 1（发现公告，不是网络失败）：14 affected package nodes，5 moderate / 9 high；**不是14个独立漏洞**。`via` 去重后为51个 GHSA。@vitest/ui、vue 等是传递影响节点；同一 GHSA 在 mocker/vitest 上不重复计数。389 dependency nodes（prod 102/dev 288 等字段有重叠，不能直接相加）。
- `cargo audit --version` 报 `no such command: audit`，没有安装工具。替代为 OSV 官方 API 对 Cargo.lock 中734个 registry package/version 查询（另1个本地根包不外发）；命中33个包版本、27个包名，52返回ID含别名，正文按41项去重列示。OSV 结果包含 RustSec/GHSA 别名、unmaintained 和 unsound 信息，不能等同于33个可利用漏洞。
- Windows `cargo tree --locked --offline --target x86_64-pc-windows-msvc -e normal,build` 已成功执行，区别了实际选入和仅锁存在。没有构建、没有抢占唯一 Rust 构建执行者，没有下载 crate/model/archive。
- 下表“修复”是公告给出的首个修复版本，不是已经验证可安全发布。兼容线更新仍须定向锁更新、回归和独立审查。没有把 dev-only、锁存在、上游 high 直接定为本项目 P1；本轮不赋项目严重性，也不关闭22。

## 证据来源与读法

1. npm registry audit 为实际锁匹配来源。每个 `GHSA-...` 原始公告 URL 为 `https://github.com/advisories/<ID>`；本文发布时间、修复版本另由 `https://api.osv.dev/v1/vulns/<ID>` 核对，OSV 返回 GitHub advisory-database 来源/别名。没有将包聚合 range 当成每个公告 range。
2. Rust 查询：POST `https://api.osv.dev/v1/querybatch`，body 为 `{"queries":[{"package":{"ecosystem":"crates.io","name":"<lock name>"},"version":"<lock version>"},...]}`；逐个 GET `https://api.osv.dev/v1/vulns/<ID>` 获取 published、affected.ranges、aliases、summary、details、database_specific。RustSec 页面为 `https://rustsec.org/advisories/<ID>.html`。表中 RustSec 日期取 published 的 UTC 日期（通常12:00Z），不是 OSV modified 日期。GHSA 日期是 GHSA published，不是 CVE/NVD 日期。
3. 对 GitHub 的 `gh api /advisories/GHSA-82fw-gwwq-j7x9 ...` 尝试未及时返回，不作为已成功证据；后续使用 OSV 的公开结构化原始公告镜像。没有声称逐页核验 GitHub 网页或运行 cargo-audit。
4. 下列 ID 表与“依赖路径/影响判断”表通过包名连接；每行继承该包的实际锁版本、路径、prod/dev/build 分类和项目判断。Rust 多版本明确列出，不隐藏旧副本。所有日期为2026年，显式标2024/2025者除外。

## npm 路径、输入边界与修复单元

锁 node 均在 `node_modules/<包名>`（scope 保留）。`dev:false` 只说明依赖图分类，不保证进入 WebView 产物。

|包/实际锁版本|路径（根→）和运行面|本项目影响判断/前置|兼容线修复目标/票|
|---|---|---|---|
|vite 6.4.1|根 devDependency；亦为 vitest dependency；开发服务器、构建工具|`vite.config.ts:17–24` host 默认为 false，可由 TAURI_DEV_HOST 扩大暴露；Playwright 明确127.0.0.1。文件读取/编辑器攻击需开发服务运行并有攻击者请求，loopback 不等于免疫。Windows UNC/命令参数问题与本平台相关；非Windows路径问题不是当前Windows复现。生产使用 dist，不内置 dev server。|6.4.4，同主版本；29|
|postcss 8.5.8；nanoid 3.3.11；source-map-js 1.2.1|vite→postcss→nanoid/source-map-js；另 vue→@vue/compiler-sfc→postcss，compiler-core/compiler-sfc/css-tree→source-map-js。三者锁 dev:false，实际主要构建链|恶意 CSS/sourceMappingURL、indexed map、异常 size 才触发对应问题。应用 src 无直接 nanoid/source-map-js 调用，未发现将翻译文本交给 CSS 编译；构建不可信 PR/插件仍存在边界。PostCSS XSS 需将不可信 CSS 不转义嵌入 style。不可只因 dev:false 判运行期漏洞。|8.5.23 / 3.3.18 / 1.2.2；均不需主版本升级；29（共享构建图）|
|vitest 4.1.4；@vitest/mocker 4.1.4；@vitest/ui 4.1.4|根 dev→vitest→mocker；根 dev→@vitest/ui|需要测试/browser/mock 服务暴露与攻击者注册/redirect。`vitest.config.ts` 默认 node；package test 为 vitest run，watch 为 vitest，无显式 browser 配置。未证实日常命令开启漏洞所需 endpoint，但不以 dev-only 免除更新。|Vitest/mocker/UI 统一4.1.12或更高4.x（核对配套精确依赖），无需5.x；30|
|fflate 0.8.2|根 dev→@vitest/ui→fflate|畸形 ZIP64 输入到 unzipSync 才触发。应用没有该 API 的直接调用；UI/报告处理攻击面尚未动态复现。|0.8.3，同0.8线；30|
|vue 3.5.29；@vue/server-renderer 3.5.29|根 prod→vue→server-renderer|SSR 属性名带 CR 且来自不可信输入时XSS。`src/main.ts:1,15` 是 createApp，src 搜索无 createSSRApp/renderToString/server-renderer；当前桌面CSR不具有已证实SSR入口，非P1结论。保持Vue家族一致版本以避免renderer mismatch。|3.5.42，无需主版本升级；31|
|yaml 2.8.2|根 prod→vue-router→yaml（Vite亦声明可选peer）|深度嵌套 YAML 导致栈耗尽。src 无直接 YAML 解析；路由构建/工具路径与实际浏览器bundle需回归确认，不能仅凭prod标记判远程可达。|2.8.3，同主版本；32|
|brace-expansion 2.0.2|根 dev→vue-tsc→@vue/language-core→minimatch；亦 @vue/test-utils→js-beautify→glob/editorconfig→minimatch→brace-expansion|恶意glob/brace字符串触发CPU、内存或栈耗尽；构建/测试配置和文件模式为主要输入。未找到用户翻译文本到glob路径。|2.1.7，同主版本；33|
|js-cookie 3.0.5|根 dev→@vue/test-utils→js-beautify→js-cookie|需攻击者控制对象属性进入 assign 并导致原型劫持/属性注入。不是应用cookie管理依赖，src无直接调用；需核验beautifier是否实际加载该路径。|OSV首个修复3.0.7；audit range <=3.0.5不能推断3.0.6已修复；34|
|undici 7.25.0|根 dev→jsdom→undici|jsdom mount测试的网络/WebSocket实现；不是Rust翻译HTTP客户端，也不是Node24内置undici版本的证据。SOCKS5/自定义TLS/cache/retry/cookie/body/ws特性各有独立前置；本仓库未证实开启这些特性。恶意测试页面/远端响应仍可影响开发进程。|7.29.1，无需8.x；35|

### npm 每项公告（实际受影响区间来自 audit，日期/修复来自 OSV）

|包|公告ID|published UTC日期|受影响区间（当前线）|首修（当前线）/触发点|
|---|---|---|---|---|
|mocker/vitest|GHSA-82fw-gwwq-j7x9|09-08|>=2.1.0 <4.1.11|4.1.11；redirect mock 路径遍历|
|mocker|GHSA-qxg6-89mg-rqv8|10-21|>=2.1.0-beta.7 <4.1.12|4.1.12；未鉴权manual mock代码注入|
|server-renderer|GHSA-g2v6-rqmx-r4w6|10-05|<3.5.42|3.5.42；SSR属性名CR|
|brace-expansion|GHSA-f886-m6hf-6m8v|03-26|>=2.0.0 <2.0.3|2.0.3；zero-step|
|brace-expansion|GHSA-3jxr-9vmj-r5cp|07-20|>=2.0.0 <2.1.2|2.1.2；连续空组指数展开|
|brace-expansion|GHSA-mh99-v99m-4gvg|07-24|>=2.0.0 <2.1.3|2.1.3；无界展开长度|
|brace-expansion|GHSA-rgw5-rvv9-x895|08-03|>=2.0.0 <2.1.4|2.1.4；中间数组绕过|
|brace-expansion|GHSA-6j4f-fj2g-mc7p|09-29|>=2.0.0 <2.1.5|2.1.5；parseCommaParts递归|
|brace-expansion|GHSA-qhr7-859c-m2p7|09-29|>=2.0.0 <2.1.6|2.1.6；嵌套组递归|
|brace-expansion|GHSA-q2hr-2g5m-vwhr|09-29|>=2.0.0 <2.1.7|2.1.7；rewrite二次复杂度|
|fflate|GHSA-px8p-9vwx-vf98|07-22|>=0.8.0 <0.8.3|0.8.3；ZIP64无限循环|
|js-cookie|GHSA-qjx8-664m-686j|05-21|<=3.0.5|3.0.7；assign原型劫持|
|nanoid|GHSA-28wg-ghj8-5hjv|07-29|<3.3.16|3.3.16；负size无限循环|
|nanoid|GHSA-2v37-7h3g-55p8|07-29|<3.3.18|3.3.18；custom size=0|
|nanoid|GHSA-xwg4-73v4-xw9w|09-01|<3.3.12|3.3.12；整数溢出|
|postcss|GHSA-qx2v-qp2m-jg93|04-24|<8.5.10|8.5.10；style结束标记XSS|
|postcss|GHSA-6g55-p6wh-862q|07-23|<=8.5.11|8.5.12；sourceMappingURL读文件|
|postcss|GHSA-r28c-9q8g-f849|07-24|<=8.5.17|8.5.18；previous map路径遍历|
|postcss|GHSA-fxqj-rqcc-2cmp|08-03|<=8.5.22|8.5.23；from缺失时修复不完整|
|source-map-js|GHSA-68fv-2mgg-jv7q|09-18|>=1.0.0 <1.2.2|1.2.2；indexed section offset DoS|
|vite|GHSA-4w7w-66w2-5vf9|04-06|<=6.4.1|6.4.2；optimized deps map遍历|
|vite|GHSA-p9ff-h696-f583|04-06|>=6.0.0 <=6.4.1|6.4.2；dev WebSocket任意读取|
|vite|GHSA-v6wh-96g9-6wx3|06-15|<=6.4.2|6.4.3；Windows UNC/NTLMv2|
|vite|GHSA-fx2h-pf6j-xcff|06-15|<=6.4.2|6.4.3；Windows fs.deny绕过|
|vite|GHSA-698v-4r3x-3vcv|10-20|<=6.4.3|6.4.4；editor %转义遗漏|
|vite|GHSA-r825-w8mm-5pxw|10-20|<=6.4.3|6.4.4；editor CLI flag误解释|
|vite|GHSA-587h-7wmj-h5pm|10-20|<=6.4.3|6.4.4；Windows existsSync绕过|
|vite|GHSA-9jrq-w75r-8gcw|10-21|<=6.4.3|6.4.4；custom middleware跨源脚本|
|vite|GHSA-rq7h-c2jc-7f22|10-21|<=6.4.3|6.4.4；非Windows allow路径匹配|
|yaml|GHSA-48c2-rrv3-qjmp|03-25|>=2.0.0 <2.8.3|2.8.3；深层collection栈溢出|
|undici|GHSA-vmh5-mc38-953g|06-18|>=7.23.0 <7.28.0|7.28.0；SOCKS5 requestTls丢失|
|undici|GHSA-p88m-4jfj-68fv|06-19|>=7.0.0 <7.28.0|7.28.0；Set-Cookie百分号解码注入|
|undici|GHSA-vxpw-j846-p89q|06-19|>=7.0.0 <7.28.0|7.28.0；WebSocket fragment计数|
|undici|GHSA-hm92-r4w5-c3mj|06-19|>=7.23.0 <7.28.0|7.28.0；SOCKS5 pool跨源路由|
|undici|GHSA-g8m3-5g58-fq7m|06-19|>=7.0.0 <7.28.0|7.28.0；SameSite子串匹配|
|undici|GHSA-pr7r-676h-xcf6|06-18|>=7.0.0 <7.28.0|7.28.0；共享cache空白绕过|
|undici|GHSA-35p6-xmwp-9g52|06-19|>=7.0.0 <7.28.0|7.28.0；keepalive响应队列污染|
|undici|GHSA-8xcm-r25x-g524|08-03|>=7.0.0 <7.29.0|7.29.0；retry响应失步|
|undici|GHSA-4cwx-7wf7-3272|08-03|>=7.0.0 <7.29.0|7.29.0；private cache指令泄漏/崩溃|
|undici|GHSA-m8rv-5g2x-5cg5|08-03|>=7.0.0 <7.29.0|7.29.0；blob type CRLF|
|undici|GHSA-jr45-8vmc-qm54|08-03|>=7.0.0 <7.29.0|7.29.0；Cache-Control等号空白|
|undici|GHSA-v3r7-h72x-cjcm|08-03|>=7.0.0 <7.29.0|7.29.0；cookie属性注入|
|undici|GHSA-pmjh-fq2x-6v4x|09-29|>=7.11.0 <7.29.1|7.29.1；RetryHandler孤立body|
|undici|GHSA-r53p-7pc4-xj5r|09-29|>=7.0.0 <7.29.1|7.29.1；retry响应拆分|
|undici|GHSA-rfgv-xxqx-mfg5|09-29|>=7.0.0 <7.29.1|7.29.1；意外WebSocket subprotocol|
|undici|GHSA-3xpg-4rpp-hhhm|09-29|>=7.15.0 <7.29.1|7.29.1；响应解压无界|
|undici|GHSA-2jfj-6hjv-fm6j|09-29|>=7.0.0 <7.29.1|7.29.1；共享cache Set-Cookie泄漏|
|undici|GHSA-2gqq-gqf2-x968|09-29|>=7.1.0 <7.29.1|7.29.1；dump chunk截断|
|undici|GHSA-w293-vg96-wgc3|09-29|>=7.24.1 <7.29.1|7.29.1；BalancedPool TLS选项丢失|
|undici|GHSA-8436-99hf-9mmv|09-29|>=7.0.0 <7.29.1|7.29.1；不安全HTTP方法cache重放|
|undici|GHSA-rx4f-c7p8-82vq|09-29|>=7.0.0 <7.29.1|7.29.1；WebSocketStream非正常关闭|

## Rust 包路径、平台与项目判断

Cargo.lock 包含所有目标/可选依赖。以下“未选入”仅针对上述 Windows normal+build feature tree，不是全平台不可达声明。非Windows未编译；不为降低audit数删除平台代码。

|包（实际锁版本）|依赖路径/分类|项目判断、修复/兼容性|
|---|---|---|
|tauri 2.10.3|根→tauri，Windows prod|GHSA origin confusion 可让远程页面调用local-only IPC；配置main/popup本地窗口、未配置remote capability，但不能因此排除框架漏洞。`tauri.conf.json:21–65` CSP=null且本地权限存在。需远程导航/页面执行前置，本轮未做真实利用。2.11.1同主版本，票36。|
|rustls 0.23.37；h2 0.4.13|根→reqwest 0.13.2→rustls；reqwest→hyper 1.8.1→h2，Windows prod|`http_client.rs:6–11` 默认TLS，连接5秒/请求15秒不等于内存上限；translator调用公网服务。TLS peer和HTTP/2响应是实际输入边界。Debug会话秘密、Brotli解压、hybrid key exchange各需额外feature/API前置，未证实全部开启。修复0.23.46/0.4.16，在现有0.x兼容线；共享网络单元37。|
|rustls-webpki 0.103.9；aws-lc-sys 0.38.0|reqwest→rustls→webpki；rustls→aws-lc-rs 1.16.1→aws-lc-sys，Windows prod选入|Windows还选入rustls-platform-verifier，实际证书策略可能使用OS，不能把每个webpki/AWS X509缺陷都判为活跃验证路径。源码没有自定义CRL；AWS-LC被用作crypto provider不证明调用其X509验证。webpki 0.103.16兼容线；aws-lc-sys 0.39.0跨0.x minor，需要由aws-lc-rs允许的版本更新，不盲目override；37只实施兼容部分并记录剩余。|
|quinn-proto 0.11.13|lock: reqwest→quinn→quinn-proto；Windows默认树未选入|HTTP/3/QUIC可选链，当前Cargo.toml只显式json/rustls，不能以lock存在声称默认QUIC端点。0.11.18兼容线；保留追踪，未建已验证可达缺陷票。|
|webpki-root-certs 1.0.6|lock: reqwest→rustls-platform-verifier→webpki-root-certs；Windows默认树未选入|平台备用根集合，不是Windows已选系统trust store。1.0.10兼容线；37验收中逐target列残留，不能假称Windows更新已修所有平台。|
|anyhow 1.0.102|根→tauri/tauri-utils/tauri-build→anyhow；另screenshots/display-info及image/ravif/rav1e，prod+build|需Error::downcast_mut相关unsound路径；src搜索无直接调用，不能排除依赖内部使用。1.0.103同主版本；38。|
|crossbeam-epoch 0.9.18|根→image→rayon/rayon-core→crossbeam-deque→epoch，Windows prod|缺陷是对invalid pointer做fmt::Pointer，不是所有并行图像操作都触发；未证明该格式化调用。0.9.20兼容线；39。|
|serde_with 3.17.0|根→tauri→tauri-utils→serde_with，prod+build|KeyValueMap空sequence/map项触发panic；src无直接KeyValueMap，不能把任意翻译空文本映射到该API。3.21.0同主版本，需核MSRV/宏配套；40。|
|rand 0.7.3、0.8.5、0.9.2|Windows0.7.3由tauri-utils→kuchikiki/selectors→phf_codegen→phf_generator（build）；0.8.5另由ppocr-rs→imageproc/rand_distr（prod）；0.9.2经image→ravif→rav1e的lock/可选链，Windows默认树未选入|需自定义logger重入rand RNG，应用tracing不是该前置的证据。0.8.6/0.9.3兼容更新，但0.7.3→0.8.6跨0.x不兼容；本轮不假称统一改rand即可解决，待上游phf链升级研究。|
|event-listener 5.4.1|lock最短路径tauri-plugin-opener 2.5.3→zbus 5.14.0→event-listener；另有async-lock/async-broadcast/async-process父节点，属于非Windows异步/D-Bus链；Windows未选入|!Send tag经StackSlot跨线程是必要条件；5.4.2同主版本。未做Linux reachability动态验证，作为后续非Windows维护，不直接P1。|
|glib 0.18.5|Tauri/wry GTK链→glib，Windows未选入|VariantStrIter迭代unsound，0.20.0跨0.x minor，需要GTK家族迁移，非小lock补丁，暂不建ready-for-agent升级票。|
|memmap2 0.7.1、0.8.0|lock: screenshots 0.8.10→libwayshot 0.2.0→memmap2 0.7.1；enigo 0.2.1→xkbcommon 0.7.0→memmap2 0.8.0；Windows均未选入|unchecked pointer offset；0.9.11跨0.x兼容边界，需要上游图形链升级，具体函数可达性待非Windows验证。|
|quick-xml 0.28.2、0.30.0、0.38.4、0.39.2|lock: screenshots→libwayshot→wayland-client 0.30.2→wayland-scanner 0.30.1→0.28.2；screenshots→xcb 1.7.0→0.30.0；tauri→plist 1.8.0→0.38.4；clipboard-manager→arboard→wl-clipboard-rs→wayland-client 0.31.13→wayland-scanner 0.31.9→0.39.2；Windows默认树四版本均未选入|不可信XML的重复attribute或namespace flood，0.41.0跨0.x边界，不能只更新单一副本。待逐平台父依赖迁移，不强加override。|
|fxhash 0.2.1|Windows截图/display-info prod；tauri-utils→kuchikiki→selectors build/prod|unmaintained，不等于公开可利用漏洞；没有patched版本，替换上游hash依赖要另外设计。|
|paste 1.0.15|Windows图像数值依赖的proc-macro|unmaintained，编译期，不是远程RCE证明；没有patched版本。|
|ttf-parser 0.25.1|ppocr-rs→imageproc→ab_glyph→owned_ttf_parser→ttf-parser，Windows prod|unmaintained，不自动等同字体解析漏洞；没有patched版本。|
|unic-char-property / unic-char-range / unic-common / unic-ucd-ident / unic-ucd-version 全部0.9.0|tauri-utils→urlpattern→unic-ucd-ident及UNIC子包，Windows prod+build|五条unmaintained同一UNIC依赖族，不作为五个高危；无patched版本，需上游urlpattern迁移。|
|proc-macro-error 1.0.4|glib-macros/gtk3-macros→proc-macro-error，Windows未选入|unmaintained，非Windows编译期；无patched版本。|
|zbus_names 4.3.1；zvariant / zvariant_derive 5.10.0|tauri-plugin-opener→zbus→名字/序列化及derive，Windows未选入|三条unmaintained，同D-Bus上游维护单元；无patched版本，不以lock删除代替维护。|

### Rust 每项公告与别名（RustSec优先去重）

区间中 `0` 表示公告所有早期版本（OSV有时用0.0.0-0），只列本项目版本所处修复线，不冒称列全其他分支。

|包|主ID（同项GHSA别名）|published日期|受影响→首修/类型|
|---|---|---|---|
|tauri|GHSA-7gmj-67g7-phm9|05-06|>=2.0.0 <2.11.1；origin confusion|
|serde_with|GHSA-7gcf-g7xr-8hxj|07-15|>=2.3.0 <3.21.0；KeyValueMap panic|
|anyhow|RUSTSEC-2026-0190|06-25|<1.0.103；downcast_mut unsound|
|aws-lc-sys|RUSTSEC-2026-0044 / GHSA-394x-vwmw-crm3|03-19（GHSA03-20）|>=0.32.0 <0.39.0；wildcard/Unicode CN约束|
|aws-lc-sys|RUSTSEC-2026-0048 / GHSA-9f94-5g5w-gf6r|03-19（GHSA03-20）|>=0.15.0 <0.39.0；CRL scope逻辑|
|crossbeam-epoch|RUSTSEC-2026-0204|07-06|>=0.9.0 <0.9.20；invalid pointer格式化|
|event-listener|RUSTSEC-2026-0221|07-13|>=5.1.0 <5.4.2；StackSlot !Send unsound|
|fxhash|RUSTSEC-2025-0057|2025-09-05|全部；unmaintained，无修复版|
|glib|RUSTSEC-2024-0429 / GHSA-wrw7-89jp-8q8g|2024-03-30（GHSA2024-12-23）|>=0.15.0 <0.20.0；VariantStrIter unsound|
|h2|RUSTSEC-2026-0258|08-17|<0.4.16；无界empty DATA frames|
|memmap2|RUSTSEC-2026-0186|06-20|<0.9.11；unchecked pointer offset unsound|
|paste|RUSTSEC-2024-0436|2024-10-07|全部；unmaintained，无修复版|
|proc-macro-error|RUSTSEC-2024-0370|2024-09-01|全部；unmaintained，无修复版|
|quick-xml|RUSTSEC-2026-0194|06-29|<0.41.0；duplicate attr二次复杂度|
|quick-xml|RUSTSEC-2026-0195|06-29|<0.41.0；NsReader namespace allocation无界|
|quinn-proto|RUSTSEC-2026-0037 / GHSA-6xvm-j4wr-6v98|03-09（GHSA03-11）|RustSec>=0.5.0 <0.11.14（GHSA introduced=0）；transport参数panic|
|quinn-proto|RUSTSEC-2026-0185 / GHSA-4w2j-m93h-cj5j|06-22（GHSA07-24）|RustSec<0.11.15（GHSA>=0.1.0）；out-of-order重组内存无界|
|quinn-proto|RUSTSEC-2026-0338|09-14|>=0.7.0 <0.11.18；零长度STREAM unordered read panic|
|quinn-proto|RUSTSEC-2026-0339|08-17|>=0.6.0 <0.11.17；connection ID retirements无界|
|quinn-proto|RUSTSEC-2026-0340|09-14|>=0.11.0 <0.11.18；ACK_FREQUENCY min delay panic|
|quinn-proto|RUSTSEC-2026-0342|08-17|>=0.5.0 <0.11.17；零长度DATAGRAM绕buffer limit|
|rand|RUSTSEC-2026-0097 / GHSA-cq8v-f236-94qc|04-09（GHSA04-14）|[0.7.0,0.8.6)、[0.9.0,0.9.3)；custom logger重入unsound|
|rustls|RUSTSEC-2026-0285 / GHSA-2mjx-qc3c-rqvc|09-14（GHSA10-05）|>=0.23.13 <0.23.45；handshake encryption level；transcript仍鉴权，不声称可篡改完成握手|
|rustls|RUSTSEC-2026-0346|10-14|>=0.21.0 <0.23.46；client session Debug泄漏TLS秘密|
|rustls|RUSTSEC-2026-0348|10-14|>=0.23.9 <0.23.46；Brotli cert解压认证前可分配1GiB|
|rustls|RUSTSEC-2026-0349|10-14|<0.23.46；ServerHello降低后续连接key exchange|
|rustls-webpki|RUSTSEC-2026-0049 / GHSA-pwjx-qhcg-rvj4|03-20（GHSA03-20）|>=0.102.0-alpha.0 <0.103.10；CRL distribution point匹配|
|rustls-webpki|RUSTSEC-2026-0098 / GHSA-965h-392x-2mh5|04-14（GHSA04-16）|RustSec<0.103.12（GHSA>=0.101.0）；URI name约束|
|rustls-webpki|RUSTSEC-2026-0099 / GHSA-xgp8-3hg3-c2mh|04-14（GHSA04-16）|RustSec<0.103.12（GHSA>=0.101.0）；wildcard name约束|
|rustls-webpki|RUSTSEC-2026-0104 / GHSA-82j2-j2ch-gfr8|04-22（GHSA04-24）|<0.103.13；malformed CRL BIT STRING panic|
|rustls-webpki|RUSTSEC-2026-0382|10-20|<0.103.16；distrusted intermediate仍可接受|
|webpki-root-certs|RUSTSEC-2026-0383|10-19|当前1.x <1.0.10（0.26线修0.26.11）；emSign distrust|
|ttf-parser|RUSTSEC-2026-0192|06-28|全部；unmaintained，无修复版|
|unic-char-range|RUSTSEC-2025-0075|2025-10-18|全部；unmaintained，无修复版|
|unic-common|RUSTSEC-2025-0080|2025-10-18|全部；unmaintained，无修复版|
|unic-char-property|RUSTSEC-2025-0081|2025-10-18|全部；unmaintained，无修复版|
|unic-ucd-version|RUSTSEC-2025-0098|2025-10-18|全部；unmaintained，无修复版|
|unic-ucd-ident|RUSTSEC-2025-0100|2025-10-18|全部；unmaintained，无修复版|
|zvariant|RUSTSEC-2026-0370|10-16|全部；unmaintained，无修复版|
|zbus_names|RUSTSEC-2026-0371|10-16|全部；unmaintained，无修复版|
|zvariant_derive|RUSTSEC-2026-0372|10-16|全部；unmaintained，无修复版|

## 已执行命令、离线验证与阻塞

命令均在 `D:/0-code-project/translate` 或显式绝对manifest路径执行。

- `npm --version` / `node --version`：11.13.0 / v24.14.1。
- `npm config get registry`：https://registry.npmjs.org/。
- `npm audit --json --package-lock-only`：exit 1，JSON 32849 bytes，14 nodes、51 unique GHSA；stderr空。临时原始结果为 `C:/Users/ZHIYU_~1/AppData/Local/Temp/npm-audit-22.json`（不作为持久仓库交付；正文已记录全部项）。
- `sha256sum package-lock.json src-tauri/Cargo.lock`：exit 0，上述快照hash。未修改依赖文件。
- `cargo audit --version`：失败，工具未安装；没有安装。
- Python标准库 `tomllib` 读取Cargo.lock，`urllib.request` POST上述querybatch：exit 0，734 queries，33 matched versions；随后 GET每个命中ID与51个npm GHSA，全部成功，无网络blocked。请求体只有ecosystem/name/version；没有发送路径、源码或私密数据。
- `cargo tree --manifest-path D:/0-code-project/translate/src-tauri/Cargo.toml --locked --offline --target x86_64-pc-windows-msvc --prefix none --format '{p}' -e normal,build`：通过Python subprocess执行exit 0。没有编译或联网。
- `cargo tree --manifest-path D:/0-code-project/translate/src-tauri/Cargo.toml --offline --locked --target x86_64-pc-windows-msvc -i <PACKAGE> --depth 5`：对 aws-lc-sys、crossbeam-epoch、h2、rand@0.7.3、rand@0.8.5、serde_with、ttf-parser、fxhash、unic-ucd-ident、anyhow 全部exit 0；`-i rustls`无depth版本亦exit 0。
- 环境间歇性 Bash 启动失败：`fatal error - add_item ("\\??\\C:\\Program Files\\Git", "/", ...) failed, errno 1`，exit 5；两次cargo-tree初次尝试及后续离线测试入口受影响。`pwd`诊断曾成功，故不是把失败当成cargo测试通过；没有杀进程/清缓存。
- 分配接口24已只读查看 `scripts/asset_manifest.py`、`scripts/AssetManifest.psm1`、`scripts/tests/test_asset_manifest.py`、`scripts/tests/Test-AssetManifest.ps1`，没有修改或发现与22相关接口缺陷。用户提供“Python5项、PS5.1/7各58拒绝用例通过”是历史证据，不记为本代理执行。
- 首次离线测试因Bash启动exit 5而NOT RUN；随后环境恢复，以下串行命令本代理实测全部exit 0：
  - `python -B -m unittest discover -s D:/0-code-project/translate/scripts/tests -p test_asset_manifest.py`：5 tests，0.010s，OK。
  - `powershell -NoProfile -ExecutionPolicy Bypass -File D:/0-code-project/translate/scripts/tests/Test-AssetManifest.ps1`：PowerShell `5.1.26100.8115`，PASS 58 shared rejection cases plus loading, selection and content checks。
  - `pwsh -NoProfile -File D:/0-code-project/translate/scripts/tests/Test-AssetManifest.ps1`：PowerShell `7.4.21`，相同58拒绝用例及加载/选择/内容检查PASS。
- 本任务没有修改PS文件，因此无新增BOM风险。Python标准库离线断言已通过：51 npm ID全部覆盖；29–40编号各唯一、ready-for-agent、含验收、Blocked by none（这些新票无依赖环）；两锁SHA-256仍与采样一致。后续Rust查询断言52返回ID全部覆盖。最终合并的覆盖/hash/空白再检查命令又遇Bash启动exit 5，NOT RUN；不以先前成功冒充最后一次执行成功。不是对全仓DAG或独立审查的替代。

### OSV 复核命令（实际执行的只读查询/覆盖检查）

```bash
PYTHONUTF8=1 python - <<'PY'
import json,tomllib,urllib.request
from pathlib import Path
root=Path('D:/0-code-project/translate')
p=tomllib.loads((root/'src-tauri/Cargo.lock').read_text())['package']
q=[{'package':{'ecosystem':'crates.io','name':x['name']},'version':x['version']}
   for x in p if x.get('source','').startswith('registry+')]
r=json.load(urllib.request.urlopen(urllib.request.Request(
    'https://api.osv.dev/v1/querybatch',data=json.dumps({'queries':q}).encode(),
    headers={'Content-Type':'application/json'}),timeout=60))
hits=[(q[i],v) for i,v in enumerate(r['results']) if v.get('vulns')]
ids={x['id'] for _,v in hits for x in v['vulns']}
report=(root/'.scratch/repository-audit/dependency-advisories.md').read_text(encoding='utf8')
assert all(i in report for i in ids)
print(len(q),len(hits),len({q['package']['name'] for q,v in hits}),len(ids))
PY
```

结果exit 0：`734 33 27 52`；逐ID详情读取为 `json.load(urllib.request.urlopen('https://api.osv.dev/v1/vulns/'+id, timeout=30))`，采集published/modified/aliases/affected/database_specific/details。数据没有落入新的生产配置或依赖文件。

## 残余风险与交接

- 未动态复现漏洞、未做恶意远端连接、未启用真实开发服务、未读取生产数据。报告给出锁命中、静态路径和前置，不把“没找到直接调用”升级为不可利用证明。
- OSV/GHSA/RustSec是采样时的数据库事实，不是永久无漏报保证；OSV查询不替代cargo-audit对source/yank等维度的检查。未来数据库可更新或撤回公告；上述别名日期差异已保留。
- 非Windows feature/target reachability、rand旧线、XML/GTK/mmap跨0.x升级和unmaintained家族仍需后续专门研究；不制造无补丁的ready-for-agent“升级即可”票。
- 29–40是明确小修复单元，全部只建票，不实施。共享package/lock及Cargo.lock是**调度互斥**，不是技术Blocked by；必须由主负责人统一串行更新并冻结验证。22维持needs-info待独立复核/主负责人裁决，map/spec未越权改动。
