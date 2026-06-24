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

  const presetEntries = Object.entries(THEME_PRESETS)
  const fontNames = Object.keys(FONT_OPTIONS)

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      closeSettings()
    }
  }
</script>

<svelte:window onkeydown={(e) => $settingsOpen && onKeydown(e)} />

{#if $settingsOpen}
  <div class="settings-overlay gw-anim-overlay" role="dialog" aria-modal="true" aria-label="Settings">
    <div class="settings-panel gw-anim-pop">
      <header class="settings-header">
        <h2>Settings</h2>
        <button class="close-btn" aria-label="Close Settings" onclick={closeSettings}>
          <Icon name="xmark" size={18} />
        </button>
      </header>

      <div class="settings-body">
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

        <section class="settings-section">
          <div class="settings-section-title">Editor — Font</div>
          <div class="settings-row">
            <div class="settings-label">Monospace font (loaded via CDN)</div>
            <select
              class="font-select"
              value={$settings.fontMono}
              onchange={(e) => setSettings({ fontMono: (e.target as HTMLSelectElement).value })}
            >
              {#each fontNames as name}
                <option value={name}>{name}</option>
              {/each}
            </select>
            <div class="font-preview" style:font-family={FONT_OPTIONS[$settings.fontMono]}>
              fn main() &#123; println!("GwenLand 123"); &#125;
            </div>
          </div>
        </section>

        <section class="settings-section">
          <AiSettingsSection />
        </section>

        <section class="settings-section">
          <LspSettingsSection />
        </section>
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-overlay {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 8vh 16px;
    background-color: rgba(0, 0, 0, 0.45);
    overflow-y: auto;
  }
  .settings-panel {
    width: 600px;
    max-width: 100%;
    background-color: var(--card);
    border: none;
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-xl);
    overflow: hidden;
  }
  /* Minimalist: no divider line — spacing alone separates the title. */
  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 20px 4px;
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
  .settings-body {
    padding: 12px 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 26px;
  }
  .settings-section-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--muted-foreground);
    margin-bottom: 14px;
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
  .font-select {
    background-color: var(--secondary);
    color: var(--foreground);
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    padding: 7px 10px;
    font-size: 13px;
    max-width: 240px;
    transition: border-color 0.12s ease;
  }
  .font-select:hover,
  .font-select:focus {
    outline: none;
    border-color: var(--border);
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
