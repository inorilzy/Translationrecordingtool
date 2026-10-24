#!/usr/bin/env node
// Cross-platform preparation of the bundled PP-OCRv6 ONNX model.
//
// Mirrors scripts/prepare_ocr_models.ps1's pinned upstream revisions and
// per-file SHA-256 checks without requiring PowerShell, so macOS and Linux
// builds use exactly the same model bytes as the Windows release.
import { createHash } from 'node:crypto'
import { mkdir, readFile, rename, rm, stat, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const SUPPORTED_PROFILE = 'small'

const FILES = ['inference.onnx', 'inference.json', 'inference.yml']

// Pinned Hugging Face revisions and SHA-256 digests from the official
// PaddlePaddle PP-OCRv6 small det/rec ONNX repositories.
const SOURCES = {
  det: {
    revision: '28fe5895c24fd108c19eb3e8479f4ab385fbfc62',
    sizes: [9880512, 229307, 885],
    sha256: [
      'd73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e',
      '89240f689a4a77aad75ef55a8df0a15c8e1d4980a327d17e58f24bbadde5aeab',
      '193f435274bf9f0b5f71a929bbfbcf148282df7e633b34e7c373e8f44741b516',
    ],
  },
  rec: {
    revision: 'b8f84f0b80c529de40b4fbb3544b84fa7233a513',
    sizes: [21159378, 208004, 150579],
    sha256: [
      '5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634',
      'f0bf53c853937a917affdd74467472167727f8ab0f0f7bded01c4a16c27e46e6',
      'ab078671bb49f06228eadccd34f1bb501e157f7a047095ffb943ba81512c77d1',
    ],
  },
}

function parseArgs(argv) {
  const options = { profile: SUPPORTED_PROFILE, force: false }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === '--force' || arg === '-Force') {
      options.force = true
    } else if (arg === '--profile' || arg === '-Profile') {
      options.profile = argv[index + 1]
      index += 1
    } else if (arg.startsWith('--profile=')) {
      options.profile = arg.slice('--profile='.length)
    } else if (arg === '--help' || arg === '-h') {
      console.log('Usage: node scripts/prepare_ocr_models.mjs [--profile small] [--force]')
      process.exit(0)
    } else {
      throw new Error(`unknown argument: ${arg}`)
    }
  }
  if (options.profile !== SUPPORTED_PROFILE) {
    throw new Error(`unsupported OCR model profile '${options.profile}' (only '${SUPPORTED_PROFILE}' is supported)`)
  }
  return options
}

async function sha256(path) {
  const digest = createHash('sha256')
  digest.update(await readFile(path))
  return digest.digest('hex')
}

async function verify(path, expectedSize, expectedSha) {
  let info
  try {
    info = await stat(path)
  } catch {
    return false
  }
  if (info.size !== expectedSize) return false
  return (await sha256(path)) === expectedSha
}

async function download(url, target) {
  const response = await fetch(url, { redirect: 'follow' })
  if (!response.ok) {
    throw new Error(`download failed (${response.status} ${response.statusText}): ${url}`)
  }
  const buffer = Buffer.from(await response.arrayBuffer())
  await mkdir(dirname(target), { recursive: true })
  await writeFile(target, buffer)
}

async function prepare(options) {
  const targetRoot = join(ROOT, 'src-tauri', 'resources', 'ocr-models', options.profile)
  for (const [role, source] of Object.entries(SOURCES)) {
    const repo = `PaddlePaddle/PP-OCRv6_${options.profile}_${role}_onnx`
    const targetDir = join(targetRoot, role)
    for (let index = 0; index < FILES.length; index += 1) {
      const name = FILES[index]
      const expectedSize = source.sizes[index]
      const expectedSha = source.sha256[index]
      const target = join(targetDir, name)
      if (!options.force && (await verify(target, expectedSize, expectedSha))) {
        console.log(`[cache] ${target}`)
        continue
      }

      const url = `https://huggingface.co/${repo}/resolve/${source.revision}/${name}`
      const temporary = `${target}.download-${process.pid}`
      console.log(`[download] ${url}`)
      try {
        await download(url, temporary)
        const actual = await sha256(temporary)
        if (actual !== expectedSha) {
          throw new Error(`SHA-256 mismatch for ${name}: expected ${expectedSha}, got ${actual}`)
        }
        const actualSize = (await stat(temporary)).size
        if (actualSize !== expectedSize) {
          throw new Error(`size mismatch for ${name}: expected ${expectedSize}, got ${actualSize}`)
        }
        // Windows does not replace an existing destination with rename.
        await rm(target, { force: true })
        await rename(temporary, target)
        console.log(`[ok] ${target}`)
      } finally {
        await rm(temporary, { force: true })
      }
    }
  }
  console.log(`[done] PP-OCRv6 ${options.profile} model ready at ${targetRoot}`)
}

try {
  await prepare(parseArgs(process.argv.slice(2)))
} catch (error) {
  console.error(`[ocr-models] ${error.message}`)
  process.exit(1)
}
