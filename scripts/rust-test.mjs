import { spawnSync } from 'node:child_process'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

export function testEnvironment(env) {
  const config = JSON.parse(env.TAURI_CONFIG || '{}')
  if (!config || Array.isArray(config) || typeof config !== 'object') {
    throw new Error('TAURI_CONFIG 必须是 JSON 对象')
  }
  if (config.bundle !== undefined && (config.bundle === null || Array.isArray(config.bundle) || typeof config.bundle !== 'object')) {
    throw new Error('TAURI_CONFIG.bundle 必须是 JSON 对象')
  }
  return {
    ...env,
    TAURI_CONFIG: JSON.stringify({ ...config, bundle: { ...config.bundle, resources: [] } }),
  }
}

export function runRustTest(args, env = process.env, spawn = spawnSync) {
  const [command = 'test', ...rest] = args
  if (!['test', 'check'].includes(command)) {
    throw new Error('测试入口仅允许 cargo test 或 cargo check；发布构建请使用正式入口')
  }
  return spawn('cargo', [command, '--manifest-path', 'src-tauri/Cargo.toml', ...rest], {
    cwd: root,
    env: testEnvironment(env),
    stdio: 'inherit',
    shell: false,
  })
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const result = runRustTest(process.argv.slice(2))
    if (result.error) {
      console.error('无法执行 Rust 测试命令:', result.error.code || '启动失败')
      process.exitCode = 1
    } else if (result.signal) {
      process.kill(process.pid, result.signal)
    } else {
      process.exitCode = result.status ?? 1
    }
  } catch {
    console.error('Rust 测试入口失败：请检查子命令及 TAURI_CONFIG JSON 对象配置')
    process.exitCode = 1
  }
}
