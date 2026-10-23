# 28 本轮集成回归、DAG/状态一致性与独立审查

Type: task
Status: resolved
Severity: n/a (集成验证，不是缺陷)
Blocked by: 25, 26, 27
Owner suggestion: 主负责人 + 唯一 Rust 构建执行者 + 独立 reviewer
Source: 批准计划“验证和关闭门禁”

## 当前结论 / Answer（2026-10-22）
依据用户提供的最终真实证据置resolved：取消人工审批后Python25通过；PS5.1/7模型各21、ORT各45通过；独立默认路径review PASS。主代理最终串行全树npm181/build/E2E1通过，Node wrapper+dependency smoke11通过，Rust check exit0、110+4+15=129通过，git diff --check exit0。见 [最新验证](../final-verification.md)。

## Comments（最新）
最终41票为37resolved+4wontfix、0待办；04–07是取消强制审批而非通过。依赖保持25/26/27，文档机械校验单列map。真实资产下载/GUI/原生信号/跨平台 NOT RUN；Rust14版本公告残余保留，不宣称无漏洞。本次只同步文档，未重新实施或测试构建。

## 历史默认路径派发状态（2026-10-22）
本票曾重新 claimed，等待25–27新默认路径测试与复核，依赖收缩为25、26、27。此前03/09/14/20–24/29–41关闭证据保留，不重复开放无关票。
04–07 已取消独立审批/正式资产背书要求并置 wontfix，不是验收通过，也不再列“等待人工信任材料”。24只保留可选严格校验器。
本轮需核对：无 manifest 默认官方固定版本且不读 release-assets.json；模型官方 SHA256；ORT无hash时大小/唯一ZIP条目/PE版本；词典结构检查及revision缓存；全部失败保旧；显式manifest严格模式仍可用且不要求用户审批。接收实际命令、退出码、测试数及独立复核后再裁决，未收到的新测试一律待验证。机械核对文档不等于产品通过；真实推理/安装包及其他未测原生边界继续明确未验证。

## 历史记录（以下验收范围、依赖、resolved与计数仅对应旧冻结树）

## 说明
本票汇总本轮所有实际实施票的最终冻结树验证。05/06/07 **不在** Blocked by 中：它们的最终验收依赖人工 04，04 未完成时 28 仍可执行，但必须在报告中如实列出 05–07 为“等待人工信任材料”，不得计入已完成。22 若派生依赖修复票，由负责人追加到本票 Blocked by。

## 验收
1. 冻结树执行 `vue-tsc --noEmit`、Vitest、前端 build、E2E；通过 23 的入口执行 Rust 定向测试、`cargo check --tests` 与全量测试。构建统一在 C 盘目标目录（由调用者配置 CARGO_TARGET_DIR），单执行者串行。记录实际命令、exit code、用例数，不沿用历史 134/181 计数。
2. 反向门禁：无测试覆盖配置时，正式 tauri 配置仍因缺正式资源失败；不伪造 dictionary.db/模型。
3. 每张实施票由非实现者 reviewer 读实际 diff，对照原票验收裁决；不由实现者自审关闭。
4. 机械核验：编号唯一连续、Blocked by 引用存在、无 self-edge/环、Status 合法、map 索引与票面一致、本地链接可解析。Bash 不可用时如实记录为人工校验。
5. GUI/原生多窗口、真实 Win32 剪贴板、非 Windows target 等环境缺失单列 NOT RUN，不写 PASS。
6. 最终报告区分：已完成并有真实证据 / 等待人工信任材料（04→05–07）/ 环境阻塞 / 22 证据状态。

## 非目标
不修改源码修 bug（发现问题另回原票或新建票），不 commit/push，不为清零改状态。

## Comments
2026-10-22：按批准计划新建；未执行。

## Answer
负责人依据 [最终验证](../final-verification.md) 批准本轮集成关闭：最终181前端测试、build/E2E、Rust129项与check通过；正式资源负向门禁仍exit101。依赖已纳入22及29–41，机制与指定补丁关闭不替代04–07人工信任/真实资产验收。

## Comments
2026-10-22 最终裁决：执行和独立审查证据见 [final-verification.md](../final-verification.md)，原计数/失败保留为历史。23 Windows真实POSIX信号、36–41真实origin/GUI/HTTP2及真实模型/安装包 NOT RUN；14非Windows NOT RUN；22 Rust仍14版本公告，不称全部修复。文档机械校验单列于map，不伪称产品测试由文档维护者重跑。

2026-10-22 文档校验补录：Python只读校验exit0，41票连续唯一、34边有效无环，37 resolved/1 ready-for-human/3 ready-for-agent，当前map状态/依赖一致，116本地Markdown链接目标有效，本轮25票Answer及最终证据链接齐全。首次Bash启动exit5后恢复，只有嵌套正则SyntaxWarning；历史失败未删。
