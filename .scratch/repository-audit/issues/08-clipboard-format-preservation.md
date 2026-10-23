# 08 划词回退不得破坏原剪贴板格式

Type: task
Status: resolved
Severity: P2
Blocked by: none
Owner suggestion: runtime / Windows clipboard adapter
Source: R1

## 顶部当前结论（2026-10-22）

**已关闭（resolved）**：方案A的白名单文本族原始字节快照/恢复已静态复核PASS，并经稳定树 `clipboard::` 10 passed、`shortcut_handler::` 7 passed、全量126 passed/0 failed、`cargo check --tests` exit0；负责人授权关闭。**不构成完整剪贴板保护**：09的备份—恢复竞争与enigo错误吞噬仍未修复，真实Win32往返未测；票内延迟渲染阻塞、部分Set失败不可回滚等边界继续有效。

## 根因与证据
`shortcut_handler.rs:315-331` 只 read_text().ok() 备份，`:295-312` 仅 Some 文本写回；`clipboard.rs:1-14` 仅文本API。UIA失败/空选区后正常可达。runtime 抽取实际恢复函数模拟原图片得到 None、复制后仍为selection，执行成功；triager重读确认，未接触系统剪贴板。

## 严重性与反证
P2：用户图片/文件/HTML/RTF内容被局部破坏，但不属于广泛灾难性数据丢失。纯文本成功恢复不反驳多格式丢失；UIA正常路径无此副作用。

## 范围 / 涉及文件
`src-tauri/src/clipboard.rs`、`shortcut_handler.rs` 及必要Windows API依赖声明。建立保真快照能力；保守最小方案为无法完整备份时在Ctrl+C之前明确失败，保留原内容，而非静默降成纯文本。能够证明完整保真后才允许fallback。与09顺序实现避免同文件冲突，但不是语义依赖。

## 验收与回归
生产adapter seam合成测试：纯文本、多格式文本、图片/文件、不支持/延迟渲染格式、备份失败；不能备份时不得发送Ctrl+C，支持时往返各格式一致。隔离Windows原生测试再验证OS实际格式保真，未跑明确blocked；不可拿字符串mock证明所有格式支持。

## 非目标
不改UIA优先级、不读取真实剪贴板；09的外部复制竞争单独处理，不因本票快照成功而声称并发安全。

## Comments
2026-10-22：采纳；建议与09同一负责人、两次小提交。
2026-10-22：已按负责人批准的选项A实现，修改 `clipboard.rs`、`shortcut_handler.rs`、`Cargo.toml`（仅在现有 windows-sys 0.59.0 追加 `Win32_System_Memory`，未新增crate/未升级）。
方案：先用 OpenClipboard 完整枚举全部格式再判定；仅的标准文本族 {CF_TEXT=1, CF_OEMTEXT=7, CF_UNICODETEXT=13, CF_LOCALE=16} 且至少一个文本格式允许，按原顺序逐格式保存原始 HGLOBAL 字节（含 EOF 终止符校验、单份≤16MiB），恢复时按原格式写回而不从 Unicode 重编码；空剪贴板作为 Empty 快照正常支持，空串文本也有效。出现 HTML/RTF、图片/DIB/GDI、CF_HDROP、未知/私有/owner-display/隐私控制注册格式、枚举或读取失败、载荷/分配异常时，在 Ctrl+C 之前返回错误，不请求任何未知格式的数据。恢复先在 EmptyClipboard 前分配并校验全部内存，成功 SetClipboardData 后清空本地句柄（所有权移交系统），失败或未处理项由 RAII 释放；恢复使用 main 窗口真实 HWND，窗口不可用同样在复制前失败。错误经 `protected_clipboard_copy` 传播到既有失败弹窗的安全文案，不再静默降级。
已知边界（不作为已修复）：白名单标准文本的 GetClipboardData 也可能同步触发 owner 延迟渲染并阻塞，未加超时线程、未发送 WM_RENDERALL，无法保证不触发 owner 回调；EmptyClipboard 之后部分 SetClipboardData 失败不可事务回滚，会返回错误但可能只恢复部分格式，UI 不承诺完整恢复；未处理外部程序在备份与恢复之间的复制竞争（09）、未处理 enigo 按键模拟错误被 `.ok` 吞掉（09）；未运行真实 Win32 剪贴板往返测试。
测试证据：`cargo test --offline --locked --manifest-path src-tauri/Cargo.toml clipboard::` 10 passed、`shortcut_handler::` 7 passed（新树串行，exit0；覆盖文本族原字节/顺序/locale 的 snapshot→prepare→transfer 成功恢复、Empty 清空一次零 Set、白名单拒绝前零数据请求、owner/坏载荷失败、预分配失败零 Empty 且释放、部分 Set 失败仅释放未移交句柄、备份失败零复制、恢复失败不假成功、UIA 成功不访问剪贴板）。全量 126 passed/0 failed，`cargo check --tests` exit0。仅合成数据，未触碰真实剪贴板。Triager 静态复核 PASS；Status 待复核员确认。
