#!/usr/bin/env node
// Package the already-built macOS .app without Finder automation.
//
// Tauri's built-in DMG bundler drives Finder via AppleScript, which times out
// in headless/CI sessions and on machines without Automation permission. This
// script creates an equivalent DMG with hdiutil and a portable ZIP with ditto.
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, rm } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const RELEASE = join(ROOT, 'src-tauri', 'target', 'release')
const APP = join(RELEASE, 'bundle', 'macos', 'translation-tool.app')
const OUT_DIR = join(RELEASE, 'bundle', 'dmg')

const config = JSON.parse(await readFile(join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'))
const arch = process.arch === 'arm64' ? 'aarch64' : 'x64'
const baseName = `translation-tool_${config.version}_${arch}`

function run(command, args) {
  execFileSync(command, args, { stdio: 'inherit' })
}

await mkdir(OUT_DIR, { recursive: true })

// Re-sign ad-hoc so the bundle's resource seal stays consistent after Tauri
// copied the models, dictionary and ONNX Runtime dylib into Contents/Resources.
run('codesign', [
  '--force',
  '--sign',
  '-',
  join(APP, 'Contents', 'Resources', 'binaries', 'onnxruntime.dylib'),
])
run('codesign', ['--force', '--deep', '--sign', '-', APP])
run('codesign', ['--verify', '--deep', '--strict', APP])

const zipPath = join(OUT_DIR, `${baseName}-portable.zip`)
await rm(zipPath, { force: true })
run('ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', APP, zipPath])

const staging = join(OUT_DIR, '.dmg-staging')
await rm(staging, { recursive: true, force: true })
await mkdir(staging, { recursive: true })
run('cp', ['-R', APP, join(staging, 'translation-tool.app')])
run('ln', ['-s', '/Applications', join(staging, 'Applications')])

const dmgPath = join(OUT_DIR, `${baseName}.dmg`)
await rm(dmgPath, { force: true })
run('hdiutil', [
  'create',
  '-volname',
  'translation-tool',
  '-srcfolder',
  staging,
  '-ov',
  '-format',
  'UDZO',
  dmgPath,
])
await rm(staging, { recursive: true, force: true })

console.log(`[macos] app:  ${APP}`)
console.log(`[macos] zip:  ${zipPath}`)
console.log(`[macos] dmg:  ${dmgPath}`)
