import assert from 'node:assert/strict'
import { test } from 'node:test'
import { mkdtemp, writeFile, rm, realpath } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createServer as createHttpServer } from 'node:http'
import { once } from 'node:events'
import postcss from 'postcss'
import sourceMap from 'source-map-js'
import { nanoid } from 'nanoid'
import { parse } from 'yaml'
import expand from 'brace-expansion'
import { zipSync, unzipSync, strToU8, strFromU8 } from 'fflate'
import { fetch } from 'undici'
import { JSDOM } from 'jsdom'
import Cookies from 'js-cookie'
import { createSSRApp, h } from 'vue'
import { renderToString } from '@vue/server-renderer'
import { createServer } from 'vite'
import { chromium } from '@playwright/test'

test('更新后的 CSS 与 source map 构建依赖保持兼容', async () => {
  const result = await postcss([]).process('.synthetic { color: red; }', {
    from: 'synthetic.css', to: 'synthetic.out.css', map: { inline: false },
  })
  assert.match(result.css, /color: red/)
  const consumer = new sourceMap.SourceMapConsumer(result.map.toJSON())
  assert.equal(consumer.originalPositionFor({ line: 1, column: 0 }).source, 'synthetic.css')
  assert.equal(nanoid(12).length, 12)
})

test('YAML、glob 与压缩测试工具保持兼容', () => {
  assert.deepEqual(parse('name: synthetic\ncount: 2\n'), { name: 'synthetic', count: 2 })
  assert.deepEqual(expand('test-{a,b}.ts'), ['test-a.ts', 'test-b.ts'])
  const unpacked = unzipSync(zipSync({ 'synthetic.txt': strToU8('synthetic fixture') }))
  assert.equal(strFromU8(unpacked['synthetic.txt']), 'synthetic fixture')
})

test('Vue SSR 家族与 cookie 测试依赖保持兼容', async () => {
  assert.equal(await renderToString(createSSRApp({ render: () => h('span', 'synthetic') })), '<span>synthetic</span>')
  const dom = new JSDOM('', { url: 'https://synthetic.invalid/' })
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'document')
  try {
    Object.defineProperty(globalThis, 'document', { configurable: true, value: dom.window.document })
    Cookies.set('synthetic', 'safe-value')
    assert.equal(Cookies.get('synthetic'), 'safe-value')
    Cookies.remove('synthetic')
    assert.equal(Cookies.get('synthetic'), undefined)
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'document', descriptor)
    else delete globalThis.document
    dom.window.close()
  }
})

test('Undici 仅向隔离本地服务发送测试请求', async () => {
  const server = createHttpServer((_request, response) => {
    response.setHeader('Content-Type', 'application/json')
    response.end('{"fixture":"synthetic"}')
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  try {
    const response = await fetch(`http://127.0.0.1:${server.address().port}/`)
    assert.deepEqual(await response.json(), { fixture: 'synthetic' })
  } finally {
    server.closeAllConnections()
    await new Promise((resolve) => server.close(resolve))
  }
})

test('Vite loopback 服务和浏览器 HMR 使用隔离临时项目', { timeout: 30000 }, async () => {
  const root = (await realpath(await mkdtemp(join(tmpdir(), 'translate-vite-smoke-')))).replaceAll('\\', '/')
  let server
  let browser
  try {
    const entry = join(root, 'main.js')
    await writeFile(join(root, 'index.html'), '<div id="app"></div><script type="module" src="/main.js"></script>')
    const code = (text) => `document.querySelector('#app').textContent = '${text}'; if (import.meta.hot) import.meta.hot.accept();`
    await writeFile(entry, code('synthetic-before'))
    server = await createServer({ configFile: false, root, logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false, fs: { allow: [root] } } })
    await server.listen()
    browser = await chromium.launch({ headless: true })
    const page = await browser.newPage()
    const response = await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/`)
    assert.equal(response.status(), 200)
    await page.waitForFunction(() => document.querySelector('#app').textContent === 'synthetic-before')
    await writeFile(entry, code('synthetic-after'))
    await page.waitForFunction(() => document.querySelector('#app').textContent === 'synthetic-after')
  } finally {
    if (browser) await browser.close()
    if (server) await server.close()
    await rm(root, { recursive: true, force: true })
  }
})
