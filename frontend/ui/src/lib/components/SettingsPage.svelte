<script lang="ts">
  import { settingsOpen, closeSettings } from '../stores/ui'
  import {
    settings,
    setSettings,
    THEME_PRESETS,
    ACCENT_POOL,
    FONT_OPTIONS,
  } from '../stores/settings'
  import Icon from './Icon.svelte'
  import AiSettingsSection from './AiSettingsSection.svelte'
  import LspSettingsSection from './LspSettingsSection.svelte'
  import CustomDropdown from './CustomDropdown.svelte'

  const presetEntries = Object.entries(THEME_PRESETS)
  const fontNames = Object.keys(FONT_OPTIONS)
  const fontItems = fontNames.map((name) => ({ value: name, label: name }))

  // GWEN-323: search filters which sections are visible. Each section declares a
  // title + keyword string; an empty query shows everything.
  let query = $state('')
  let searchInput = $state<HTMLInputElement>()

  // Section visibility keyed by a stable id. `keywords` is matched (with the
  // title) case-insensitively against the query.
  const SECTIONS = [
    { id: 'theme', title: 'Appearance — Theme', keywords: 'appearance theme preset accent color dark swatch' },
    { id: 'font', title: 'Editor — Font', keywords: 'editor font monospace family typeface code' },
    { id: 'ai', title: 'AI', keywords: 'ai provider model api key anthropic openai assistant training' },
    { id: 'lsp', title: 'Language Servers', keywords: 'lsp language server rust typescript python diagnostics completion' },
  ] as const

  function matches(id: string): boolean {
    const q = query.trim().toLowerCase()
    if (!q) return true
    const sec = SECTIONS.find((s) => s.id === id)
    if (!sec) return true
    return (sec.title + ' ' + sec.keywords).toLowerCase().includes(q)
  }
  const anyMatch = $derived(SECTIONS.some((s) => matches(s.id)))

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      closeSettings()
    }
  }

  // Focus the search box and reset the query whenever the panel opens.
  $effect(() => {
    if ($settingsOpen) {
      query = ''
      queueMicrotask(() => searchInput?.focus())
    }
  })
</script>

<svelte:window onkeydown={(e) => $settingsOpen && onKeydown(e)} />

{#if $settingsOpen}
  <!-- Scrim: click outside the panel dismisses (GWEN-323). -->
  <div
    class="settings-scrim gw-anim-overlay"
    role="presentation"
    onclick={closeSettings}
  ></div>
  <div class="settings-panel" role="dialog" aria-modal="true" aria-label="Settings">
    <header class="settings-header">
      <h2>Settings</h2>
      <button class="close-btn" aria-label="Close Settings" onclick={closeSettings}>
        <Icon name="xmark" size={18} />
      </button>
    </header>

    <div class="settings-search">
      <Icon name="search" size={14} class="search-icon" />
      <input
        bind:this={searchInput}
        bind:value={query}
        type="text"
        class="search-input"
        placeholder="Search settings..."
        aria-label="Search settings"
        spellcheck="false"
      />
    </div>

    <div class="settings-body">
      {#if matches('theme')}
        <section class="settings-section">
          <div class="settings-section-title">Appearance — Theme</div>

          <div class="settings-row">
            <div class="settings-label">Preset</div>
            <div class="settings-presets">
              {#each presetEntries as [key, preset]}
                <button
                  type="button"
                  class="preset-chip"
                  class:selected={$settings.preset === key}
                  onclick={() => setSettings({ preset: key })}
                >
                  <span
                    class="preset-swatch"
                    style:background-color={preset.vars['--background']}
                    style:border-color={preset.vars['--border']}
                  ></span>
                  {preset.label}
                </button>
              {/each}
            </div>
          </div>

          <div class="settings-row">
            <div class="settings-label">Accent color</div>
            <div class="settings-accents">
              {#each ACCENT_POOL as color}
                <button
                  type="button"
                  class="accent-swatch"
                  class:selected={$settings.accent.toLowerCase() === color.toLowerCase()}
                  style:background-color={color}
                  aria-label={`Accent ${color}`}
                  title={color}
                  onclick={() => setSettings({ accent: color })}
                ></button>
              {/each}
              <label class="accent-custom" title="Custom accent">
                <input
                  type="color"
                  value={$settings.accent}
                  oninput={(e) => setSettings({ accent: (e.target as HTMLInputElement).value })}
                />
              </label>
            </div>
          </div>
        </section>
      {/if}

      {#if matches('font')}
        <section class="settings-section">
          <div class="settings-section-title">Editor — Font</div>
          <div class="settings-row">
            <div class="settings-label">Monospace font (loaded via CDN)</div>
            <div class="font-dropdown-wrap">
              <CustomDropdown
                items={fontItems}
                value={$settings.fontMono}
                onSelect={(v) => setSettings({ fontMono: v })}
                label="Monospace font"
              />
            </div>
            <div class="font-preview" style:font-family={FONT_OPTIONS[$settings.fontMono]}>
              fn main() &#123; println!("GwenLand 123"); &#125;
            </div>
          </div>
        </section>
      {/if}

      {#if matches('ai')}
        <section class="settings-section">
          <AiSettingsSection />
        </section>
      {/if}

      {#if matches('lsp')}
        <section class="settings-section">
          <LspSettingsSection />
        </section>
      {/if}

      {#if !anyMatch}
        <div class="settings-empty">No settings match “{query}”.</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .settings-scrim {
    position: fixed;
    inset: 0;
    z-index: 90;
    background-color: rgba(0, 0, 0, 0.35);
  }
  /* GWEN-323: docked to the right edge, slides in. */
  .settings-panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 91;
    width: 460px;
    max-width: 100%;
    background-color: var(--card);
    border-left: 1px solid var(--border);
    box-shadow: var(--shadow-xl);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: settings-slide-in 0.18s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes settings-slide-in {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }
  @media (prefers-reduced-motion: reduce) {
    .settings-panel {
      animation: none;
    }
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 20px 10px;
    flex-shrink: 0;
  }
  .settings-header h2 {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--foreground);
  }
  .close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    line-height: 1;
    cursor: pointer;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
  }
  .close-btn:hover {
    color: var(--foreground);
    background-color: var(--secondary);
  }

  /* Search bar (sticky at the top of the scroll area's header region). */
  .settings-search {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 20px 8px;
    padding: 8px 12px;
    background-color: var(--secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }
  .settings-search :global(.search-icon) {
    color: var(--muted-foreground);
  }
  .search-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .search-input::placeholder {
    color: var(--muted-foreground);
  }

  .settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 26px;
  }
  /* GWEN-323: sticky section headers on scroll. */
  .settings-section-title {
    position: sticky;
    top: 0;
    z-index: 1;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--muted-foreground);
    margin-bottom: 14px;
    padding: 8px 0 6px;
    background-color: var(--card);
  }
  .settings-empty {
    font-size: 13px;
    color: var(--muted-foreground);
    padding: 12px 0;
  }
  .settings-row {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 18px;
  }
  .settings-label {
    font-size: 13px;
    color: var(--foreground);
  }
  .settings-presets {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .preset-chip {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    background-color: transparent;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--muted-foreground);
    font-size: 12px;
    cursor: pointer;
    transition: border-color 0.15s ease, background-color 0.15s ease, color 0.15s ease;
  }
  .preset-chip:hover {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .preset-chip.selected {
    color: var(--foreground);
    border-color: var(--primary);
    background-color: color-mix(in srgb, var(--primary) 12%, transparent);
  }
  .preset-swatch {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1px solid var(--border);
  }
  .settings-accents {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
  }
  .accent-swatch {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
    transition: transform 0.12s ease, box-shadow 0.15s ease;
  }
  .accent-swatch:hover {
    transform: scale(1.15);
  }
  .accent-swatch.selected {
    border-color: var(--foreground);
    box-shadow: 0 0 0 2px var(--background), 0 0 0 3px var(--foreground);
  }
  .accent-custom {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px dashed var(--muted-foreground);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    cursor: pointer;
  }
  .accent-custom input[type='color'] {
    width: 36px;
    height: 36px;
    border: none;
    padding: 0;
    background: transparent;
    cursor: pointer;
  }
  .font-dropdown-wrap {
    max-width: 240px;
    min-width: 160px;
  }
  .font-preview {
    margin-top: 4px;
    padding: 12px 14px;
    background-color: var(--background);
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--muted-foreground);
  }
</style>
