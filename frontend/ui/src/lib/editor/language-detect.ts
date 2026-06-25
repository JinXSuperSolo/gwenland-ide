import type { Extension } from '@codemirror/state'
import {
  defaultHighlightStyle,
  StreamLanguage,
  syntaxHighlighting,
  type StreamParser,
  type StringStream,
} from '@codemirror/language'

type LanguageId =
  | 'javascript'
  | 'rust'
  | 'python'
  | 'css'
  | 'html'
  | 'json'
  | 'markdown'

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
        return C_LIKE_KEYWORDS.has(word) ? 'keyword' : 'variableName'
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
        return PY_KEYWORDS.has(word) ? 'keyword' : 'variableName'
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
      return /[{}:;,]/.test(ch) ? 'punctuation' : null
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
      if (ch === '"') return eatString(stream, ch)
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
    case 'md':
    case 'mdx':
      return 'markdown'
    default:
      return null
  }
}

export function getLanguageExtension(filename: string): Extension {
  const id = languageIdForFilename(filename)
  return id ? [syntaxHighlighting(defaultHighlightStyle, { fallback: true }), LANGUAGES[id]] : []
}
