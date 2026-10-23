import assert from 'node:assert/strict'
import { test } from 'node:test'
import { runRustTest, testEnvironment } from './rust-test.mjs'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, writeFileSync, readFileSync, rmSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, delimiter } from 'node:path'
import { fileURLToPath } from 'node:url'

test('资源覆盖只进入子进程并保留其他设置', () => {
  const env = {
    CARGO_TARGET_DIR: 'C:/temporary-target',
    TAURI_CONFIG: JSON.stringify({ productName: 'synthetic', bundle: { active: true, resources: ['missing.db'] } }),
    EXTRA: 'preserved',
  }
  const before = { ...env }
  const actual = testEnvironment(env)
  assert.deepEqual(JSON.parse(actual.TAURI_CONFIG), { productName: 'synthetic', bundle: { active: true, resources: [] } })
  assert.equal(actual.CARGO_TARGET_DIR, env.CARGO_TARGET_DIR)
  assert.equal(actual.EXTRA, 'preserved')
  assert.deepEqual(env, before)
})

test('无配置时只禁用测试资源复制', () => {
  assert.deepEqual(JSON.parse(testEnvironment({}).TAURI_CONFIG), { bundle: { resources: [] } })
})

test('非法配置拒绝且不启动 cargo', () => {
  for (const config of ['broken', 'null', '[]', '7', '{"bundle":null}', '{"bundle":[]}']) {
    assert.throws(() => runRustTest(['test'], { TAURI_CONFIG: config }, () => assert.fail('不应启动')))
  }
})

test('命令参数作为数组传递，保留退出码与信号', () => {
  for (const outcome of [{ status: 0, signal: null }, { status: 17, signal: null }, { status: null, signal: 'SIGTERM' }, { error: { code: 'ENOENT' } }]) {
    const actual = runRustTest(['check', '--tests', '--locked'], {}, (command, args, options) => {
      assert.equal(command, 'cargo')
      assert.deepEqual(args, ['check', '--manifest-path', 'src-tauri/Cargo.toml', '--tests', '--locked'])
      assert.equal(options.shell, false)
      assert.equal(options.stdio, 'inherit')
      assert.deepEqual(JSON.parse(options.env.TAURI_CONFIG), { bundle: { resources: [] } })
      return outcome
    })
    assert.equal(actual, outcome)
  }
})

test('真实CLI通过隔离fake cargo传播退出码并拒绝非法配置', () => {
  const dir = mkdtempSync(join(tmpdir(), 'translate-rust-wrapper-'))
  try {
    const source = join(dir, 'fake.rs')
    const binary = join(dir, process.platform === 'win32' ? 'cargo.exe' : 'cargo')
    const evidence = join(dir, 'evidence.txt')
    writeFileSync(source, `fn main() {
      let args = std::env::args().skip(1).collect::<Vec<_>>().join("\\n");
      let config = std::env::var("TAURI_CONFIG").unwrap();
      let target = std::env::var("CARGO_TARGET_DIR").unwrap();
      std::fs::write(std::env::var("FAKE_CARGO_EVIDENCE").unwrap(), format!("{}\\n{}\\n{}", config, target, args)).unwrap();
      std::process::exit(std::env::var("FAKE_CARGO_EXIT").unwrap().parse().unwrap());
    }`)
    const compile = spawnSync('rustc', ['--crate-name', 'fake_cargo', source, '-o', binary], { encoding: 'utf8', shell: false })
    assert.equal(compile.status, 0, compile.stderr || String(compile.error))
    const env = { ...process.env }
    const pathKey = Object.keys(env).find((key) => key.toLowerCase() === 'path') || 'PATH'
    env[pathKey] = dir + delimiter + (env[pathKey] || '')
    env.TAURI_CONFIG = JSON.stringify({ productName: 'synthetic', bundle: { active: true, resources: ['missing'] } })
    env.CARGO_TARGET_DIR = join(dir, 'target')
    env.FAKE_CARGO_EVIDENCE = evidence
    const cli = fileURLToPath(new URL('./rust-test.mjs', import.meta.url))
    for (const code of [0, 17]) {
      env.FAKE_CARGO_EXIT = String(code)
      const child = spawnSync(process.execPath, [cli, 'check', '--tests', '--locked'], { env, encoding: 'utf8', shell: false })
      assert.equal(child.status, code, child.stderr)
      const [config, target, ...args] = readFileSync(evidence, 'utf8').split('\n')
      assert.deepEqual(JSON.parse(config), { productName: 'synthetic', bundle: { active: true, resources: [] } })
      assert.equal(target, env.CARGO_TARGET_DIR)
      assert.deepEqual(args, ['check', '--manifest-path', 'src-tauri/Cargo.toml', '--tests', '--locked'])
    }
    rmSync(evidence)
    env.TAURI_CONFIG = 'invalid SYNTHETIC_PRIVATE_CONFIG'
    const rejected = spawnSync(process.execPath, [cli, 'test'], { env, encoding: 'utf8', shell: false })
    assert.equal(rejected.status, 1)
    assert.equal(existsSync(evidence), false)
    assert.ok(!rejected.stderr.includes('SYNTHETIC_PRIVATE_CONFIG'))
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('默认test，禁止发布命令进入资源覆盖入口', () => {
  runRustTest([], {}, (_command, args) => { assert.equal(args[0], 'test'); return { status: 0 } })
  for (const command of ['build', 'run', 'install', 'publish']) {
    assert.throws(() => runRustTest([command], {}, () => assert.fail('不应启动')))
  }
})
