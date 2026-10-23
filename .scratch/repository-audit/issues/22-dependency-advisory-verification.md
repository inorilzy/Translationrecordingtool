# 22 核实安装器漏洞汇总的包级可达性

Type: research
Status: resolved
Severity: unassigned (not a confirmed vulnerability)
Blocked by: none
Owner suggestion: security / dependency maintainer
Source: security.md dependency observation + tests.md npm ci

## 线索与反证
tests执行固定lock的npm ci --ignore-scripts提示14 vulnerabilities（5 moderate/9 high），当前没有逐项advisory/受影响版本/生产可达性证据。安装器high不能直接映射项目P1；dev-only也不能未经检查一概忽略。

## 待取证 / 涉及文件
固定 `package-lock.json`、`package.json`，在授权网络/镜像可信环境取得audit JSON与时间、advisory原文；按包路径、运行期/构建期/开发server、受影响功能和输入可信边界核实。Cargo advisories另标覆盖缺口，不混称本次14项。

## 验收与回归
每项包含受影响实际锁版本、调用链、必要前置、反证、修复版本/兼容性和结论；去重传递链同根因后才建实现票与严重性。若无暴露则写wontfix理由；无访问证据维持needs-info。不得仅跑npm audit fix或升级所有依赖。

## 非目标
不在本轮联网查CVE、不修改锁/依赖、不将14汇总当14个可利用漏洞。

## Answer
本票关闭的是公告核实与修复单元拆分，不是所有依赖公告全部修复。负责人依据 [最终验证](../final-verification.md) 批准 resolved；29–41 指定目标已消除，当次 npm audit vulnerabilities 为空。

### Rust 剩余公告：14 个版本仍命中
最终 OSV 查询 771 个 registry 包版本，仍有 14 个版本命中：Windows 的 rand0.7.3 旧构建链及 fxhash/paste/ttf-parser 维护公告；Windows 树外 glib、memmap2、旧 quick-xml、proc-macro-error、zbus_names、zvariant/derive 等。非全部修复、非全图无公告，不以锁命中宣称项目可利用。Windows plist 链旧 XML 已由 41 修复；逐项边界见 [最终残余](../final-verification.md) 与 [原始公告报告](../dependency-advisories.md)。

## Comments
2026-10-22：审计范围未含advisory同步；列为未验证研究，不计P0/P1/P2确证数。
2026-10-22（批准计划）：本轮授权对当前 lock 获取包级 npm audit 与 RustSec 官方证据（仅发送依赖元数据，不发送凭据/翻译内容），记录公告时间、版本区间、依赖路径与生产/构建/开发影响。不运行 npm audit fix、不批量升主版本。确认可修后按实际共享依赖最小修复单元新建票（从 29 起），不预先建空票；package/lock/Cargo 变更单一 owner 串行。证据不可得时如实 blocked，不宣称无漏洞。状态保持 needs-info，尚未派发。

2026-10-22（22取证交付，未关闭）：新增 [dependency-advisories.md](../dependency-advisories.md)，记录当前锁hash、公告ID/发布时间/受影响区间/修复版本、依赖路径及prod/dev/build/目标平台前置。npm audit --json --package-lock-only返回exit 1（成功发现公告）：14包节点、51唯一GHSA；不是14个独立可利用漏洞。cargo-audit未安装，使用批准的OSV querybatch查询734 registry包版本：33版本/27包名命中，52返回ID含别名，按41项公告去重列示，包含unmaintained/unsound。Windows offline cargo tree核对实际选入；未做恶意动态复现，未赋项目P1。

2026-10-22（最小修复单元）：创建29–40共12票，均ready-for-agent、含验收、未实施：29 Vite/CSS共享图，30 Vitest/UI/fflate，31 Vue家族，32 router/yaml，33 minimatch/brace-expansion，34 test-utils/js-cookie，35 jsdom/undici，36 Tauri origin，37 reqwest TLS/h2共享图，38 anyhow，39 crossbeam-epoch，40 serde_with。共享锁文件为调度互斥，不制造技术依赖。无补丁unmaintained及glib/memmap/XML/rand旧线跨兼容边界仍待研究，不自动升主版本。

2026-10-22（验证）：最初Bash启动间歇性exit 5，后恢复。实际离线命令 `python -B -m unittest discover -s D:/0-code-project/translate/scripts/tests -p test_asset_manifest.py`：5 tests PASS；`powershell -NoProfile -ExecutionPolicy Bypass -File D:/0-code-project/translate/scripts/tests/Test-AssetManifest.ps1`：PS5.1.26100.8115，58拒绝用例及加载/选择/内容检查PASS；`pwsh -NoProfile -File D:/0-code-project/translate/scripts/tests/Test-AssetManifest.ps1`：PS7.4.21，同样PASS。两锁hash复核未变；51 npm ID及52 Rust返回ID均被报告覆盖；新票编号唯一/状态/验收断言通过。未修改依赖、24模块、map/spec、README或生产资产；未下载模型/归档、未安装工具、未commit/push。状态保留needs-info，由独立复核与主负责人决定后续；非Windows动态可达性、无补丁项及修复回归仍非完成。

2026-10-22 最终裁决：负责人依据 [最终执行、独立复核及残余说明](../final-verification.md) 批准公告核实票 resolved；前述 needs-info/未关闭为历史。29–41指定目标已修，Rust仍14版本公告单列于 Answer，不声称全部修复或逐漏洞可利用性已实测。
