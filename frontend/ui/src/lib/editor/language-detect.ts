import type { Extension } from '@codemirror/state'
import {
  StreamLanguage,
  syntaxHighlighting,
  type StreamParser,
  type StringStream,
} from '@codemirror/language'
import { EditorView } from '@codemirror/view'

type LanguageId =
  | 'javascript'
  | 'rust'
  | 'python'
  | 'css'
  | 'html'
  | 'json'
  | 'markdown'
  | 'toml'

interface SimpleState {
  block: string | null
}

const C_LIKE_KEYWORDS = new Set([
  'as',
  'async',
  'await',
  'break',
  'case',
  'catch',
  'class',
  'const',
  'continue',
  'default',
  'dyn',
  'else',
  'enum',
  'export',
  'extends',
  'false',
  'finally',
  'fn',
  'for',
  'from',
  'function',
  'if',
  'impl',
  'import',
  'in',
  'interface',
  'let',
  'loop',
  'match',
  'mod',
  'move',
  'mut',
  'new',
  'null',
  'of',
  'pub',
  'return',
  'self',
  'static',
  'struct',
  'super',
  'switch',
  'this',
  'throw',
  'trait',
  'true',
  'try',
  'type',
  'undefined',
  'use',
  'var',
  'where',
  'while',
])

const PY_KEYWORDS = new Set([
  'and',
  'as',
  'async',
  'await',
  'break',
  'class',
  'continue',
  'def',
  'elif',
  'else',
  'except',
  'False',
  'finally',
  'for',
  'from',
  'global',
  'if',
  'import',
  'in',
  'is',
  'lambda',
  'None',
  'nonlocal',
  'not',
  'or',
  'pass',
  'raise',
  'return',
  'self',
  'True',
  'try',
  'while',
  'with',
  'yield',
])

function eatString(stream: StringStream, quote: string): string {
  let escaped = false
  while (!stream.eol()) {
    const ch = stream.next()
    if (escaped) {
      escaped = false
    } else if (ch === '\\') {
      escaped = true
    } else if (ch === quote) {
      break
    }
  }
  return 'string'
}

function eatIdentifier(stream: StringStream): string {
  stream.eatWhile(/[\w$-]/)
  return stream.current()
}

function nextNonSpace(stream: StringStream): string {
  return stream.string.slice(stream.pos).match(/^\s*(.)/)?.[1] ?? ''
}

function classifyIdentifier(stream: StringStream, word: string): string {
  const before = stream.string.slice(0, stream.start).trimEnd()
  const next = nextNonSpace(stream)
  if (before.endsWith('.')) return next === '(' ? 'propertyName.function' : 'propertyName'
  if (/\b(fn|function|def)\s+$/.test(before)) return 'variableName.function'
  if (next === '(') return 'variableName.function'
  if (/^[A-Z]/.test(word)) return 'typeName'
  return 'variableName'
}

const gwenHighlighter = {
  style(tags: readonly unknown[]): string | null {
    const names = tags
      .map((tag) => (tag as { name?: string }).name ?? '')
      .filter(Boolean)
    const has = (needle: string) => names.some((name) => name.includes(needle))
    if (has('invalid')) return 'gw-syn-invalid'
    if (has('comment')) return 'gw-syn-comment'
    if (has('keyword')) return 'gw-syn-keyword'
    if (has('string')) return 'gw-syn-string'
    if (has('tagName')) return 'gw-syn-tag'
    if (has('attributeName')) return 'gw-syn-attribute'
    if (has('typeName') || has('className')) return 'gw-syn-type'
    if (has('propertyName')) return 'gw-syn-property'
    if (has('function')) return 'gw-syn-function'
    if (has('number') || has('literal') || has('atom') || has('bool')) return 'gw-syn-number'
    if (has('operator') || has('punctuation')) return 'gw-syn-operator'
    if (has('variableName')) return 'gw-syn-variable'
    if (has('heading')) return 'gw-syn-heading'
    return null
  },
}

const gwenSyntaxTheme = EditorView.theme({
  '.gw-syn-keyword': { color: '#d68d5c' },
  '.gw-syn-string': { color: '#89b96e' },
  '.gw-syn-comment': { color: '#5a5a5a', fontStyle: 'italic' },
  '.gw-syn-function': { color: '#dcdcaa' },
  '.gw-syn-type': { color: '#4ec9b0' },
  '.gw-syn-number': { color: '#b5cea8' },
  '.gw-syn-variable': { color: '#9cdcfe' },
  '.gw-syn-operator': { color: '#cccccc' },
  '.gw-syn-property': { color: '#ce9178' },
  '.gw-syn-tag': { color: '#f28b55' },
  '.gw-syn-attribute': { color: '#d7ba7d' },
  '.gw-syn-heading': { color: '#d68d5c', fontWeight: '600' },
  '.gw-syn-invalid': { color: '#f87171' },
})

function cLikeParser(name: LanguageId): StreamParser<SimpleState> {
  return {
    name,
    startState: () => ({ block: null }),
    languageData: { commentTokens: { line: '//', block: { open: '/*', close: '*/' } } },
    token(stream, state) {
      if (state.block) {
        if (stream.skipTo(state.block)) {
          stream.match(state.block)
          state.block = null
        } else {
          stream.skipToEnd()
        }
        return 'comment'
      }
      if (stream.eatSpace()) return null
      if (stream.match('//')) {
        stream.skipToEnd()
        return 'comment'
      }
      if (stream.match('/*')) {
        state.block = '*/'
        return 'comment'
      }
      const ch = stream.next() ?? ''
      if (ch === '"' || ch === "'" || ch === '`') return eatString(stream, ch)
      if (/\d/.test(ch)) {
        stream.eatWhile(/[0-9a-fA-FxX._]/)
        return 'number'
      }
      if (/[A-Za-z_$]/.test(ch)) {
        const word = eatIdentifier(stream)
        return C_LIKE_KEYWORDS.has(word) ? 'keyword' : classifyIdentifier(stream, word)
      }
      if (/[+\-*/%=!<>|&^~?:]/.test(ch)) {
        stream.eatWhile(/[+\-*/%=!<>|&^~?:]/)
        return 'operator'
      }
      return /[{}()[\];,.]/.test(ch) ? 'punctuation' : null
    },
  }
}

function pythonParser(): StreamParser<SimpleState> {
  return {
    name: 'python',
    startState: () => ({ block: null }),
    languageData: { commentTokens: { line: '#' } },
    token(stream) {
      if (stream.eatSpace()) return null
      if (stream.match('#')) {
        stream.skipToEnd()
        return 'comment'
      }
      const ch = stream.next() ?? ''
      if (ch === '"' || ch === "'") return eatString(stream, ch)
      if (/\d/.test(ch)) {
        stream.eatWhile(/[0-9a-fA-FxX._]/)
        return 'number'
      }
      if (/[A-Za-z_]/.test(ch)) {
        const word = eatIdentifier(stream)
        return PY_KEYWORDS.has(word) ? 'keyword' : classifyIdentifier(stream, word)
      }
      if (/[+\-*/%=!<>|&^~]/.test(ch)) {
        stream.eatWhile(/[+\-*/%=!<>|&^~]/)
        return 'operator'
      }
      return /[{}()[\]:,.]/.test(ch) ? 'punctuation' : null
    },
  }
}

function cssParser(): StreamParser<SimpleState> {
  return {
    name: 'css',
    startState: () => ({ block: null }),
    languageData: { commentTokens: { block: { open: '/*', close: '*/' } } },
    token(stream, state) {
      if (state.block) {
        if (stream.skipTo(state.block)) {
          stream.match(state.block)
          state.block = null
        } else {
          stream.skipToEnd()
        }
        return 'comment'
      }
      if (stream.eatSpace()) return null
      if (stream.match('/*')) {
        state.block = '*/'
        return 'comment'
      }
      const ch = stream.next() ?? ''
      if (ch === '"' || ch === "'") return eatString(stream, ch)
      if (ch === '.' || ch === '#') {
        stream.eatWhile(/[\w-]/)
        return 'className'
      }
      if (/[A-Za-z-]/.test(ch)) {
        stream.eatWhile(/[\w-]/)
        return stream.peek() === ':' ? 'propertyName' : 'tagName'
      }
      if (/\d/.test(ch)) {
        stream.eatWhile(/[0-9.%a-zA-Z-]/)
        return 'number'
      }
      return /[{}:;,]/.test(ch) ? 'punctuation' : 'operator'
    },
  }
}

function htmlParser(): StreamParser<SimpleState> {
  return {
    name: 'html',
    startState: () => ({ block: null }),
    languageData: { commentTokens: { block: { open: '<!--', close: '-->' } } },
    token(stream, state) {
      if (state.block) {
        if (stream.skipTo(state.block)) {
          stream.match(state.block)
          state.block = null
        } else {
          stream.skipToEnd()
        }
        return 'comment'
      }
      if (stream.eatSpace()) return null
      if (stream.match('<!--')) {
        state.block = '-->'
        return 'comment'
      }
      if (stream.match(/^<\/?[\w:-]+/)) return 'tagName'
      if (stream.match(/^[\w:-]+(?=\=)/)) return 'attributeName'
      const ch = stream.next() ?? ''
      if (ch === '"' || ch === "'") return eatString(stream, ch)
      return ch === '<' || ch === '>' || ch === '/' || ch === '=' ? 'punctuation' : null
    },
  }
}

function jsonParser(): StreamParser<SimpleState> {
  return {
    name: 'json',
    startState: () => ({ block: null }),
    token(stream) {
      if (stream.eatSpace()) return null
      const ch = stream.next() ?? ''
      if (ch === '"') {
        eatString(stream, ch)
        return nextNonSpace(stream) === ':' ? 'propertyName' : 'string'
      }
      if (/\d|-/.test(ch)) {
        stream.eatWhile(/[0-9.eE+-]/)
        return 'number'
      }
      if (/[A-Za-z]/.test(ch)) {
        const word = eatIdentifier(stream)
        return word === 'true' || word === 'false' || word === 'null' ? 'atom' : null
      }
      return /[{}[\]:,]/.test(ch) ? 'punctuation' : null
    },
  }
}

function tomlParser(): StreamParser<SimpleState> {
  return {
    name: 'toml',
    startState: () => ({ block: null }),
    languageData: { commentTokens: { line: '#' } },
    token(stream) {
      if (stream.eatSpace()) return null
      if (stream.match('#')) {
        stream.skipToEnd()
        return 'comment'
      }
      if (stream.sol() && stream.match(/^\s*\[[^\]]+\]/)) return 'heading'
      const ch = stream.next() ?? ''
      if (ch === '"' || ch === "'") return eatString(stream, ch)
      if (/\d|-/.test(ch)) {
        stream.eatWhile(/[0-9.eE+_\-]/)
        return 'number'
      }
      if (/[A-Za-z_]/.test(ch)) {
        const word = eatIdentifier(stream)
        if (word === 'true' || word === 'false') return 'atom'
        return nextNonSpace(stream) === '=' ? 'propertyName' : 'variableName'
      }
      if (ch === '=') return 'operator'
      return /[{}[\],.]/.test(ch) ? 'punctuation' : null
    },
  }
}

function markdownParser(): StreamParser<SimpleState> {
  return {
    name: 'markdown',
    startState: () => ({ block: null }),
    token(stream) {
      if (stream.sol() && stream.match(/^#{1,6}\s.*/)) return 'heading'
      if (stream.sol() && stream.match(/^>\s.*/)) return 'quote'
      if (stream.match(/^```.*$/)) return 'processingInstruction'
      if (stream.match(/^\*\*[^*]+\*\*/)) return 'strong'
      if (stream.match(/^`[^`]+`/)) return 'monospace'
      stream.next()
      return null
    },
  }
}

const LANGUAGES: Record<LanguageId, Extension> = {
  javascript: StreamLanguage.define(cLikeParser('javascript')).extension,
  rust: StreamLanguage.define(cLikeParser('rust')).extension,
  python: StreamLanguage.define(pythonParser()).extension,
  css: StreamLanguage.define(cssParser()).extension,
  html: StreamLanguage.define(htmlParser()).extension,
  json: StreamLanguage.define(jsonParser()).extension,
  markdown: StreamLanguage.define(markdownParser()).extension,
  toml: StreamLanguage.define(tomlParser()).extension,
}

export function languageIdForFilename(filename: string): LanguageId | null {
  const ext = filename.split('.').pop()?.toLowerCase()
  switch (ext) {
    case 'ts':
    case 'tsx':
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
      return 'javascript'
    case 'rs':
      return 'rust'
    case 'py':
      return 'python'
    case 'css':
    case 'scss':
      return 'css'
    case 'html':
    case 'svelte':
    case 'xml':
      return 'html'
    case 'json':
    case 'jsonl':
      return 'json'
    case 'toml':
      return 'toml'
    case 'md':
    case 'mdx':
      return 'markdown'
    default:
      return null
  }
}

export function getLanguageExtension(filename: string): Extension {
  const id = languageIdForFilename(filename)
  return id ? [gwenSyntaxTheme, syntaxHighlighting(gwenHighlighter, { fallback: true }), LANGUAGES[id]] : []
}
