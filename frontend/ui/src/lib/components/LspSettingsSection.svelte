<script lang="ts">
  import { onMount } from 'svelte'
  import {
    loadEngineSettings,
    saveEngineSettings,
    type EngineSettings,
    type LanguageServerSettings,
  } from '../tauri/commands'
  import Checkbox from './Checkbox.svelte'

  /**
   * Minimal LSP settings (Milestone 6): per-language enable toggle, command, and
   * args. Leaving `command` blank uses the built-in default (shown as the
   * placeholder). No secrets are stored. Missing servers are surfaced in the
   * status bar, not here (Requirement 4.x / 12.3).
   */

  type LangKey = 'rust' | 'typescript' | 'python'
  const LANGS: { key: LangKey; label: string; defCmd: string; defArgs: string }[] = [
    { key: 'rust', label: 'Rust', defCmd: 'rust-analyzer', defArgs: '' },
    {
      key: 'typescript',
      label: 'TypeScript / JavaScript',
      defCmd: 'typescript-language-server',
      defArgs: '--stdio',
    },
    { key: 'python', label: 'Python', defCmd: 'pyright-langserver', defArgs: '--stdio' },
  ]

  let settings = $state<EngineSettings | null>(null)
  let savedNote = $state('')

  onMount(async () => {
    try {
      settings = await loadEngineSettings()
    } catch {
      settings = null
    }
  })

  function ls(key: LangKey): LanguageServerSettings | null {
    return settings ? settings.lsp[key] : null
  }

  async function persist() {
    if (!settings) return
    try {
      await saveEngineSettings(settings)
      savedNote = 'Saved'
      setTimeout(() => (savedNote = ''), 1800)
    } catch (e) {
      savedNote = String(e)
    }
  }

  function onToggle(key: LangKey, enabled: boolean) {
    if (!settings) return
    settings.lsp[key].enabled = enabled
    void persist()
  }
  function onCommand(key: LangKey, e: Event) {
    if (!settings) return
    settings.lsp[key].command = (e.target as HTMLInputElement).value.trim()
    void persist()
  }
  function onArgs(key: LangKey, e: Event) {
    if (!settings) return
    settings.lsp[key].args = (e.target as HTMLInputElement).value.split(/\s+/).filter(Boolean)
    void persist()
  }
</script>

<div class="settings-section-title">Language Servers (LSP)</div>

{#if settings}
  <p class="lsp-hint">
    GwenLand connects to language servers you install yourself. Leave a command
    blank to use the default. Servers that aren't installed show as
    <em>no server</em> in the status bar — editing still works.
  </p>

  {#each LANGS as lang}
    {@const cfg = ls(lang.key)}
    {#if cfg}
      <div class="lsp-lang">
        <div class="lsp-enable">
          <Checkbox checked={cfg.enabled} onCheck={(v) => onToggle(lang.key, v)}>
            <span class="lsp-lang-name">{lang.label}</span>
          </Checkbox>
        </div>
        <div class="lsp-fields" class:disabled={!cfg.enabled}>
          <input
            class="lsp-input"
            type="text"
            value={cfg.command}
            placeholder={lang.defCmd}
            aria-label={`${lang.label} command`}
            disabled={!cfg.enabled}
            onchange={(e) => onCommand(lang.key, e)}
          />
          <input
            class="lsp-input lsp-args"
            type="text"
            value={cfg.args.join(' ')}
            placeholder={lang.defArgs || '(no args)'}
            aria-label={`${lang.label} arguments`}
            disabled={!cfg.enabled}
            onchange={(e) => onArgs(lang.key, e)}
          />
        </div>
      </div>
    {/if}
  {/each}

  {#if savedNote}<div class="lsp-saved">{savedNote}</div>{/if}
{:else}
  <p class="lsp-hint">Could not load settings.</p>
{/if}

<style>
  .settings-section-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--muted-foreground);
    margin-bottom: 10px;
  }
  .lsp-hint {
    font-size: 12px;
    color: var(--muted-foreground);
    margin-bottom: 14px;
    line-height: 1.5;
  }
  .lsp-lang {
    margin-bottom: 14px;
  }
  .lsp-enable {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    margin-bottom: 6px;
  }
  .lsp-lang-name {
    font-size: 13px;
    color: var(--foreground);
    font-weight: 600;
  }
  .lsp-fields {
    display: flex;
    gap: 8px;
  }
  .lsp-fields.disabled {
    opacity: 0.5;
  }
  .lsp-input {
    background-color: var(--input);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 6px 9px;
    font-size: 12px;
    font-family: var(--font-mono);
  }
  .lsp-input:first-child {
    flex: 2;
  }
  .lsp-args {
    flex: 1;
  }
  .lsp-saved {
    font-size: 11px;
    color: var(--chart-2, #3fb950);
    margin-top: 6px;
  }
</style>
