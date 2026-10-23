# 30 同步 Vitest/mocker/UI 兼容补丁与 fflate

Type: task
Status: resolved
Severity: unassigned (dev/test only; exposure conditional)
Blocked by: none
Source: 22；../dependency-advisories.md
File lock: package.json/package-lock.json 单一owner串行。

## 范围
vitest、@vitest/mocker、@vitest/ui 当前4.1.4；fflate 0.8.2由UI引入。GHSA-82fw-gwwq-j7x9（redirect mock）、GHSA-qxg6-89mg-rqv8（manual mock代码注入）、GHSA-px8p-9vwx-vf98（ZIP64无限循环）。目标配套4.1.12或更高4.x，fflate >=0.8.3且保留0.8线；无需Vitest5。检查精确内部版本/peer，不能仅更新mocker override制造不匹配。
默认node/vitest run未证实暴露browser mock endpoint；仍应修复测试服务输入边界，不定P1。

## 验收
- 定向锁更新并检查所有副本，npm ls无invalid；audit逐ID不再命中对应节点。
- 全量Vitest与mount测试、watch启动/退出、已有UI功能（如未使用明确NOT RUN）回归；无开放外网监听。
- fflate以合成小ZIP/ZIP64 fixture验证正常解压及错误有界，不下载真实归档、不用无限循环fixture阻塞主进程。
- 不运行audit fix，不批量升主版本；独立review，记录实际命令/结果。

## Comments
2026-10-22：22证据拆出的同一Vitest依赖单元；仅建票，未实施。

## Answer
Vitest/UI/mocker4.1.12 与 fflate0.8.3 配套完成，真实安装树核对、ZIP 兼容 fixture 和独立 review PASS。负责人批准补丁关闭，见 [最终验证](../final-verification.md)。

## Comments
2026-10-22 最终裁决：依据 [最终实际新版 181 项回归与依赖 smoke](../final-verification.md) 置 resolved；旧安装树超时保留为历史，非逐漏洞复现或未列出的 UI/watch 实测。
