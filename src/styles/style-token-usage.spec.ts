import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SRC_ROOT = resolve(fileURLToPath(new URL('.', import.meta.url)), '..')

/** Read a file relative to src/ as raw text. */
function readSrc(relPath: string): string {
  return readFileSync(resolve(SRC_ROOT, relPath), 'utf-8')
}

// ---------------------------------------------------------------------------
// Token definitions extracted from design-tokens.css
// ---------------------------------------------------------------------------

function extractDefinedTokens(): Set<string> {
  const css = readSrc('styles/design-tokens.css')
  const tokens = new Set<string>()
  const re = /--([a-zA-Z0-9_-]+)/g
  let match: RegExpExecArray | null
  while ((match = re.exec(css)) !== null) {
    tokens.add(match[1])
  }
  return tokens
}

const DEFINED_TOKENS = extractDefinedTokens()

// ---------------------------------------------------------------------------
// Violation detectors
// ---------------------------------------------------------------------------

interface Violation {
  file: string
  category: 'raw-color' | 'raw-rgba' | 'unknown-token' | 'inline-style'
  snippet: string
}

const COMMENT_RE = /^\s*(\/\*|\*|\/\/)/

/** Detect raw #hex color literals (excluding comments and design-tokens.css). */
function detectRawColors(fileRel: string, source: string): Violation[] {
  const violations: Violation[] = []
  if (fileRel === 'styles/design-tokens.css') return violations
  const hexRe = /#[0-9a-fA-F]{3,8}\b/g
  for (const line of source.split('\n')) {
    if (COMMENT_RE.test(line)) continue
    hexRe.lastIndex = 0
    if (hexRe.test(line)) {
      violations.push({
        file: fileRel,
        category: 'raw-color',
        snippet: line.trim(),
      })
    }
  }
  return violations
}

/** Detect raw rgba() / rgb() calls outside design-tokens.css. */
function detectRawRgba(fileRel: string, source: string): Violation[] {
  const violations: Violation[] = []
  if (fileRel === 'styles/design-tokens.css') return violations
  const rgbaRe = /rgba?\s*\(/g
  for (const line of source.split('\n')) {
    if (COMMENT_RE.test(line)) continue
    if (rgbaRe.test(line)) {
      violations.push({
        file: fileRel,
        category: 'raw-rgba',
        snippet: line.trim(),
      })
    }
  }
  return violations
}

/** Detect var(--unknown-token) references not defined in design-tokens.css. */
function detectUnknownTokens(fileRel: string, source: string): Violation[] {
  const violations: Violation[] = []
  const varRe = /var\(--([a-zA-Z0-9_-]+)/g
  for (const line of source.split('\n')) {
    if (COMMENT_RE.test(line)) continue
    let match: RegExpExecArray | null
    varRe.lastIndex = 0
    while ((match = varRe.exec(line)) !== null) {
      const tokenName = match[1]
      if (!DEFINED_TOKENS.has(tokenName)) {
        violations.push({
          file: fileRel,
          category: 'unknown-token',
          snippet: `${line.trim()}  \u2192 unknown token: --${tokenName}`,
        })
      }
    }
  }
  return violations
}

/** Detect inline style="" attributes in <template> sections of Vue SFCs. */
function detectInlineStyles(fileRel: string, source: string): Violation[] {
  const violations: Violation[] = []
  if (!fileRel.endsWith('.vue')) return violations
  const tplMatch = source.match(/<template[^>]*>([\s\S]*?)<\/template>/)
  if (!tplMatch) return violations
  const inlineRe = /\bstyle\s*=\s*["']/g
  let match: RegExpExecArray | null
  inlineRe.lastIndex = 0
  while ((match = inlineRe.exec(tplMatch[1])) !== null) {
    violations.push({
      file: fileRel,
      category: 'inline-style',
      snippet: tplMatch[1]
        .slice(Math.max(0, match.index - 30), match.index + 40)
        .trim(),
    })
  }
  return violations
}

// ---------------------------------------------------------------------------
// Shared-surface file list
// ---------------------------------------------------------------------------

const SHARED_SURFACES = [
  'styles/global.css',
  'components/NavigationBar.vue',
  'components/EmptyState.vue',
]

function gatherSharedFiles(): string[] {
  const found: string[] = []
  for (const rel of SHARED_SURFACES) {
    try {
      readSrc(rel)
      found.push(rel)
    } catch {
      // file may not exist yet; skip gracefully
    }
  }
  return found
}

// ---------------------------------------------------------------------------
// Content-surface file list
// ---------------------------------------------------------------------------

const CONTENT_SURFACES = [
  'components/TranslationCard.vue',
  'views/HomePage.vue',
  'views/HistoryPage.vue',
  'views/FavoritesPage.vue',
  'views/DetailPage.vue',
  'views/SettingsPage.vue',
  'views/LogsPage.vue',
  'views/PopupWindow.vue',
]

function gatherContentFiles(): string[] {
  const found: string[] = []
  for (const rel of CONTENT_SURFACES) {
    try {
      readSrc(rel)
      found.push(rel)
    } catch {
      // file may not exist yet; skip gracefully
    }
  }
  return found
}

// ---------------------------------------------------------------------------
// Reusable violation check runner
// ---------------------------------------------------------------------------

/** Run all violation detectors against a list of files and produce named tests. */
function assertNoViolations(files: string[], suiteLabel: string) {
  describe(`no raw hex colors outside design-tokens.css (${suiteLabel})`, () => {
    for (const rel of files) {
      it(`${rel} contains no raw hex colors`, () => {
        const source = readSrc(rel)
        const violations = detectRawColors(rel, source)
        expect(
          violations,
          violations.map((v) => `${v.file}: ${v.snippet}`).join('\n'),
        ).toEqual([])
      })
    }
  })

  describe(`no raw rgba/rgb outside design-tokens.css (${suiteLabel})`, () => {
    for (const rel of files) {
      it(`${rel} contains no raw rgba/rgb`, () => {
        const source = readSrc(rel)
        const violations = detectRawRgba(rel, source)
        expect(
          violations,
          violations.map((v) => `${v.file}: ${v.snippet}`).join('\n'),
        ).toEqual([])
      })
    }
  })

  describe(`all var() references resolve to defined tokens (${suiteLabel})`, () => {
    for (const rel of files) {
      it(`${rel} has no unknown token references`, () => {
        const source = readSrc(rel)
        const violations = detectUnknownTokens(rel, source)
        expect(
          violations,
          violations.map((v) => `${v.file}: ${v.snippet}`).join('\n'),
        ).toEqual([])
      })
    }
  })

  describe(`no inline style="" in Vue templates (${suiteLabel})`, () => {
    for (const rel of files.filter((f) => f.endsWith('.vue'))) {
      it(`${rel} has no inline styles in template`, () => {
        const source = readSrc(rel)
        const violations = detectInlineStyles(rel, source)
        expect(
          violations,
          violations.map((v) => `${v.file}: ${v.snippet}`).join('\n'),
        ).toEqual([])
      })
    }
  })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('style-token-usage: shared surfaces', () => {
  const files = gatherSharedFiles()

  it('has shared surface files to scan', () => {
    expect(files.length).toBeGreaterThanOrEqual(3)
  })

  assertNoViolations(files, 'shared surfaces')
})

describe('style-token-usage: content surfaces', () => {
  const files = gatherContentFiles()

  it('has content surface files to scan', () => {
    expect(files.length).toBeGreaterThanOrEqual(5)
  })

  assertNoViolations(files, 'content surfaces')
})

describe('style-token-usage: design-tokens integrity', () => {
  it('design-tokens.css defines core semantic tokens', () => {
    const expected = [
      'color-primary',
      'color-primary-hover',
      'color-text-on-primary',
      'color-error-bg',
      'color-success-bg',
      'color-chip-bg',
      'color-chip-border',
      'radius-sm',
      'radius-md',
      'radius-full',
      'radius-pill',
      'font-size-xs',
      'font-size-icon',
      'font-weight-medium',
      'font-weight-semibold',
      'border-width',
      'transition-fast',
      'scrollbar-thumb',
      'scrollbar-thumb-light',
      'toggle-bg-inactive',
      'toggle-thumb',
    ]
    for (const token of expected) {
      expect(DEFINED_TOKENS.has(token), `missing token: --${token}`).toBe(true)
    }
  })

  it('all themes define matching token sets', () => {
    const css = readSrc('styles/design-tokens.css')
    const themes = ['light', 'dark', 'one-dark', 'github-light', 'github-dark']
    for (const theme of themes) {
      const selector = theme === 'light'
        ? ':root,'
        : `[data-theme="${theme}"]`
      expect(css).toContain(selector)
    }
  })
})
