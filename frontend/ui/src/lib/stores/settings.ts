import { writable } from 'svelte/store'

/**
 * Theme/appearance settings — dark-only presets + a layered accent + a mono
 * font choice, persisted to localStorage (NOT the engine's settings.toml — that
 * was the deliberate M2 decision and is kept here). Values are applied live to
 * CSS custom properties on :root, exactly like the legacy themeController.
 */

export interface ThemePreset {
  label: string
  vars: Record<string, string>
}

// Each preset overrides the base dark tokens; accent layers on top separately.
// "Gwen Dark" mirrors the hex baseline in styles/tokens.css.
export const THEME_PRESETS: Record<string, ThemePreset> = {
  gwen: {
    label: 'Gwen Dark',
    vars: {
      '--background': '#1f1e1e',
      '--foreground': '#fafafa',
      '--card': '#252323',
      '--popover': '#1f1e1e',
      '--secondary': '#211e1d',
      '--muted': '#211e1d',
      '--muted-foreground': '#a39e9a',
      '--border': '#282625',
      '--input': '#282625',
      '--sidebar': '#1f1e1e',
      '--sidebar-accent': '#242220',
      '--sidebar-border': '#282524',
      '--sidebar-foreground': '#e6e6e6',
    },
  },
  midnight: {
    label: 'Midnight',
    vars: {
      '--background': 'oklch(0.2050 0.0240 264.0000)',
      '--foreground': 'oklch(0.9300 0.0120 255.0000)',
      '--card': 'oklch(0.2050 0.0240 264.0000)',
      '--popover': 'oklch(0.2250 0.0260 264.0000)',
      '--secondary': 'oklch(0.2550 0.0280 264.0000)',
      '--muted': 'oklch(0.2550 0.0280 264.0000)',
      '--muted-foreground': 'oklch(0.6600 0.0220 258.0000)',
      '--border': 'oklch(0.2900 0.0300 264.0000)',
      '--input': 'oklch(0.2900 0.0300 264.0000)',
      '--sidebar': 'oklch(0.1850 0.0220 264.0000)',
      '--sidebar-accent': 'oklch(0.2700 0.0320 264.0000)',
      '--sidebar-border': 'oklch(0.2750 0.0300 264.0000)',
      '--sidebar-foreground': 'oklch(0.9100 0.0120 255.0000)',
    },
  },
  slate: {
    label: 'Slate',
    vars: {
      '--background': 'oklch(0.2400 0.0060 285.0000)',
      '--foreground': 'oklch(0.9650 0.0040 286.0000)',
      '--card': 'oklch(0.2400 0.0060 285.0000)',
      '--popover': 'oklch(0.2650 0.0070 285.0000)',
      '--secondary': 'oklch(0.2900 0.0080 286.0000)',
      '--muted': 'oklch(0.2900 0.0080 286.0000)',
      '--muted-foreground': 'oklch(0.7150 0.0090 286.0000)',
      '--border': 'oklch(0.3300 0.0090 286.0000)',
      '--input': 'oklch(0.3300 0.0090 286.0000)',
      '--sidebar': 'oklch(0.2200 0.0060 285.0000)',
      '--sidebar-accent': 'oklch(0.3050 0.0090 286.0000)',
      '--sidebar-border': 'oklch(0.3150 0.0090 286.0000)',
      '--sidebar-foreground': 'oklch(0.9500 0.0050 286.0000)',
    },
  },
}

// Accent color pool (readable on every dark preset). First entry matches the
// primary baseline (#c28a64); the rest are popular editor accents.
export const ACCENT_POOL = [
  '#c28a64', '#e0975f', '#e06c75', '#c678dd', '#61afef',
  '#56b6c2', '#98c379', '#e5c07b', '#7aa2f7', '#bb9af7',
  '#f7768e', '#73daca',
]

// Fonts loaded via Google Fonts CDN (see <link> in index.html).
export const FONT_OPTIONS: Record<string, string> = {
  'JetBrains Mono': "'JetBrains Mono', monospace",
  'Fira Code': "'Fira Code', monospace",
  'Source Code Pro': "'Source Code Pro', monospace",
  'IBM Plex Mono': "'IBM Plex Mono', monospace",
  'Roboto Mono': "'Roboto Mono', monospace",
}

export interface Settings {
  preset: string
  accent: string
  fontMono: string
}

// Bumped to v3 when the palette moved to the warm-sand hex set, so a stale
// oklch accent saved under v2 doesn't override the new defaults on upgrade.
const STORAGE_KEY = 'gwen.theme.v3'

const DEFAULTS: Settings = {
  preset: 'gwen',
  accent: '#c28a64',
  fontMono: 'JetBrains Mono',
}

/** Approximate rgb triplet of the primary, used for the resize-handle /
 *  focus-ring rgba() fallbacks when the accent isn't a plain hex. */
const BASE_RING_RGB = '194 138 100'

/** Best-effort rgb triplet for an accent. Hex is exact; oklch/other → baseline. */
function accentToRgbTriplet(accent: string): string {
  const hex = accent.trim()
  if (/^#?[0-9a-fA-F]{6}$/.test(hex)) {
    const m = hex.replace('#', '')
    const r = parseInt(m.slice(0, 2), 16)
    const g = parseInt(m.slice(2, 4), 16)
    const b = parseInt(m.slice(4, 6), 16)
    return `${r} ${g} ${b}`
  }
  return BASE_RING_RGB
}

function load(): Settings {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}')
    return {
      preset: saved.preset && THEME_PRESETS[saved.preset] ? saved.preset : DEFAULTS.preset,
      accent: typeof saved.accent === 'string' ? saved.accent : DEFAULTS.accent,
      fontMono:
        saved.fontMono && FONT_OPTIONS[saved.fontMono] ? saved.fontMono : DEFAULTS.fontMono,
    }
  } catch {
    return { ...DEFAULTS }
  }
}

/** Apply settings live to CSS custom properties on :root (the tokens). */
function applyToDom(s: Settings): void {
  const root = document.documentElement
  root.classList.add('dark')
  root.setAttribute('data-theme', 'dark')

  const preset = THEME_PRESETS[s.preset] || THEME_PRESETS.gwen
  for (const [k, v] of Object.entries(preset.vars)) root.style.setProperty(k, v)

  // Accent layered over the preset.
  root.style.setProperty('--primary', s.accent)
  root.style.setProperty('--accent', s.accent)
  root.style.setProperty('--ring', s.accent)
  root.style.setProperty('--sidebar-primary', s.accent)
  root.style.setProperty('--sidebar-accent-foreground', s.accent)
  root.style.setProperty('--ring-rgb', accentToRgbTriplet(s.accent))

  const font = FONT_OPTIONS[s.fontMono] || FONT_OPTIONS['JetBrains Mono']
  root.style.setProperty('--font-mono', font)
}

function persist(s: Settings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s))
  } catch {
    /* storage unavailable; non-fatal */
  }
}

export const settings = writable<Settings>(load())

// Apply on every change (live) + persist. The initial load() value is applied
// immediately by initSettings() below to avoid a flash before first subscribe.
settings.subscribe((s) => {
  applyToDom(s)
  persist(s)
})

/** Patch a subset of settings; triggers live re-apply + persist. */
export function setSettings(partial: Partial<Settings>): void {
  settings.update((s) => ({ ...s, ...partial }))
}

/** Apply the persisted settings to the DOM at startup (called from main.ts). */
export function initSettings(): void {
  applyToDom(load())
}
