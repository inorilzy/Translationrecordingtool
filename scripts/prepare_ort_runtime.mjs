#!/usr/bin/env node
// Cross-platform preparation of the bundled ONNX Runtime dynamic library.
//
// Downloads the official, SHA-256 pinned ONNX Runtime 1.20.1 archive for the
// current platform (Windows x64 DLL / macOS arm64 dylib) into src-tauri/binaries.
// macOS dylibs are ad-hoc signed. scripts/prepare_ort_runtime.ps1 stays available
// on Windows for the stricter manifest mode.
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
    archiveName: 'onnxruntime.tgz',
    libraryPath: `onnxruntime-osx-arm64-${ORT_VERSION}/lib/libonnxruntime.${ORT_VERSION}.dylib`,
    target: 'onnxruntime.dylib',
  },
  'win32-x64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-win-x64-${ORT_VERSION}.zip`,
    sha256: '78d447051e48bd2e1e778bba378bec4ece11191c9e538cf7b2c4a4565e8f5581',
    archiveName: 'onnxruntime.zip',
    libraryPath: `onnxruntime-win-x64-${ORT_VERSION}/lib/onnxruntime.dll`,
    target: 'onnxruntime.dll',
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
  // bsdtar (macOS, Windows 10+) extracts both .tgz and .zip archives.
  execFileSync('tar', ['-xf', archive, '-C', destination], { stdio: 'inherit' })
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
  const source = ARCHIVES[options.platform]
  if (!source) {
    throw new Error(
      `no pinned ONNX Runtime ${ORT_VERSION} archive for ${options.platform}; ` +
        `supported: ${Object.keys(ARCHIVES).join(', ')}`,
    )
  }

  const target = join(BINARIES, source.target)
  if (!options.force && (await exists(target))) {
    console.log(`[cache] ${target}`)
    return
  }

  const work = await mkdtemp(join(tmpdir(), 'ort-'))
  try {
    const archive = join(work, source.archiveName)
    console.log(`[download] ${source.url}`)
    await download(source.url, archive)
    const actual = await sha256(archive)
    if (actual !== source.sha256) {
      throw new Error(`SHA-256 mismatch: expected ${source.sha256}, got ${actual}`)
    }

    extract(archive, work)
    const extracted = join(work, ...source.libraryPath.split('/'))
    if (!(await exists(extracted))) {
      throw new Error(`archive does not contain ${source.libraryPath}`)
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
