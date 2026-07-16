# 开发版启动指南

## 标准启动（交互式终端）

在 PowerShell 或 cmd 里直接运行：

```bash
npm run tauri -- dev --config src-tauri/tauri.ocr-native.conf.json
```

- `tauri.ocr-native.conf.json` 是开发推荐配置：native ONNX OCR，不依赖 Python sidecar。
- 首次运行前需完成 README「开发环境」小节的 `npm install`、词典与 OCR 模型准备。

启动成功的判断依据（cargo 编译可能耗时数分钟，终端长时间无输出是正常的）：

- 出现主窗口「选词翻译工具」；
- Vite dev server 监听 `1420` 端口（`Get-NetTCPConnection -State Listen -LocalPort 1420`）；
- 进程列表里有 `translation-tool`（`Get-Process translation-tool`）。

## 非交互环境启动（进程管理器 / CI / 脚本 spawn）

**Windows 上 `npm` 不是可执行文件**，直接 spawn `npm` 会失败。原因：

`C:\Program Files\nodejs\` 下的 `npm` 是三个脚本，各自需要对应的解释器宿主：

| 文件 | 宿主 | 谁会用到 |
|---|---|---|
| `npm.ps1` | PowerShell | PowerShell 交互式输入 `npm` 时优先命中 |
| `npm.cmd` | cmd.exe | cmd 交互式；`spawn(..., { shell: true })` |
| `npm`（sh 脚本） | bash | Git Bash / WSL |

交互式 shell 之所以能裸敲 `npm`，是因为 shell 替你做了「按 PATH + PATHEXT 找脚本 → 选解释器执行」。而 `CreateProcess` / `child_process.spawn('npm')` 这类**直接进程创建没有 shell 参与**，只认 PE 可执行文件（.exe），于是报 `'npm' 不是内部或外部命令` 或 Node 里的 `ENOENT`。

正确做法（二选一）：

```text
# 方式一：显式给 cmd 宿主
C:\Windows\System32\cmd.exe /d /c "npm run tauri -- dev --config src-tauri/tauri.ocr-native.conf.json"

# 方式二：跳过脚本，直接调 node + npm-cli.js
"C:\Program Files\nodejs\node.exe" "C:\Program Files\nodejs\node_modules\npm\bin\npm-cli.js" run tauri -- dev --config src-tauri/tauri.ocr-native.conf.json
```

Node 脚本里等价写法是 `spawn('npm', args, { shell: true })`，或直接 spawn `npm.cmd` 的完整路径。

同样的规则适用于 `npx`、`yarn`、`pnpm`——它们在 Windows 上都是脚本，不是 exe。

## 常见失败症状对照

| 症状 | 根因 | 解法 |
|---|---|---|
| `'npm' 不是内部或外部命令` | 直接 spawn 了脚本名，无 shell 宿主 | 用上面「方式一/二」 |
| `spawn npm ENOENT`（Node） | 同上 | `shell: true` 或指 `npm.cmd` 完整路径 |
| `'"cmd.exe"' 不是内部或外部命令` | 上层工具把命令名带引号整体传下去（常见于 PTY 包装） | 关闭 PTY / 换直接 spawn 模式 |
| 启动后长时间无输出 | cargo 全量编译中 | 等待；用端口 1420 或 `translation-tool` 进程判断就绪 |
