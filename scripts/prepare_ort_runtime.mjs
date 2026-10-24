#!/usr/bin/env node
// Cross-platform preparation of the bundled ONNX Runtime dynamic library.
//
// Windows keeps using scripts/prepare_ort_runtime.ps1 (pinned x64 1.20.1 DLL).
// macOS downloads the official arm64 1.20.1 archive and publishes an ad-hoc
// signed `onnxruntime.dylib` next to the Tauri binary resources.
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { copyFile, mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const BINARIES = join(ROOT, 'src-tauri', 'binaries')

const ORT_VERSION = '1.20.1'
const ARCHIVES = {
  'darwin-arm64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-osx-arm64-${ORT_VERSION}.tgz`,
    sha256: 'b678fc3c2354c771fea4fba420edeccfba205140088334df801e7fc40e83a57a',
    library: `libonnxruntime.${ORT_VERSION}.dylib`,
    target: 'onnxruntime.dylib',
  },
}

function parseArgs(argv) {
  const options = { force: false, platform: `${process.platform}-${process.arch}` }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === '--force' || arg === '-Force') {
      options.force = true
    } else if (arg === '--platform') {
      options.platform = argv[index + 1]
      index += 1
    } else if (arg.startsWith('--platform=')) {
      options.platform = arg.slice('--platform='.length)
    } else if (arg === '--help' || arg === '-h') {
      console.log('Usage: node scripts/prepare_ort_runtime.mjs [--platform darwin-arm64] [--force]')
      process.exit(0)
    } else {
      throw new Error(`unknown argument: ${arg}`)
    }
  }
  return options
}

async function sha256(path) {
  const digest = createHash('sha256')
  digest.update(await readFile(path))
  return digest.digest('hex')
}

async function exists(path) {
  try {
    await stat(path)
    return true
  } catch {
    return false
  }
}

async function download(url, target) {
  const response = await fetch(url, { redirect: 'follow' })
  if (!response.ok) {
    throw new Error(`download failed (${response.status} ${response.statusText}): ${url}`)
  }
  await writeFile(target, Buffer.from(await response.arrayBuffer()))
}

function extract(archive, destination) {
  execFileSync('tar', ['-xzf', archive, '-C', destination], { stdio: 'inherit' })
}

function signMacLibrary(path) {
  if (process.platform !== 'darwin') return
  try {
    execFileSync('codesign', ['--force', '--sign', '-', path], { stdio: 'inherit' })
  } catch (error) {
    throw new Error(`ad-hoc codesign failed for ${path}: ${error.message}`)
  }
}

async function prepare(options) {
  if (options.platform === 'win32-x64') {
    console.log('[ort] Windows uses scripts/prepare_ort_runtime.ps1; run npm run ocr:ort:win')
    return
  }

  const source = ARCHIVES[options.platform]
  if (!source) {
    throw new Error(
      `no pinned ONNX Runtime ${ORT_VERSION} archive for ${options.platform}; ` +
        'supported: win32-x64 (PowerShell), darwin-arm64',
    )
  }

  const target = join(BINARIES, source.target)
  if (!options.force && (await exists(target))) {
    console.log(`[cache] ${target}`)
    return
  }

  const work = await mkdtemp(join(tmpdir(), 'ort-'))
  try {
    const archive = join(work, 'onnxruntime.tgz')
    console.log(`[download] ${source.url}`)
    await download(source.url, archive)
    const actual = await sha256(archive)
    if (actual !== source.sha256) {
      throw new Error(`SHA-256 mismatch: expected ${source.sha256}, got ${actual}`)
    }

    extract(archive, work)
    const extracted = join(work, `onnxruntime-osx-arm64-${ORT_VERSION}`, 'lib', source.library)
    if (!(await exists(extracted))) {
      throw new Error(`archive does not contain ${source.library}`)
    }

    await mkdir(BINARIES, { recursive: true })
    await copyFile(extracted, target)
    signMacLibrary(target)
    console.log(`[ok] ${target}`)
  } finally {
    await rm(work, { recursive: true, force: true })
  }
}

try {
  await prepare(parseArgs(process.argv.slice(2)))
} catch (error) {
  console.error(`[ort-runtime] ${error.message}`)
  process.exit(1)
}
