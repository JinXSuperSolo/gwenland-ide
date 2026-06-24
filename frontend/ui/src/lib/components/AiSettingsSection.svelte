<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './Icon.svelte'
  import Checkbox from './Checkbox.svelte'
  import {
    aiSetKey,
    aiDeleteKey,
    aiCheckKey,
    loadEngineSettings,
    saveEngineSettings,
    type EngineSettings,
    type GenericProviderSetting,
  } from '../tauri/commands'
  import { aiChat } from '../stores/ai-chat'

  /**
   * AI settings (Milestone 4): provider API keys (write-only — only stored/
   * missing status is ever shown), user-configured generic OpenAI-compatible
   * providers, and the global training opt-in toggle (explicit confirm to
   * enable). Keys go to the OS keychain via `ai_set_key`; everything else lives
   * in settings.toml `[ai]`.
   */

  const NATIVE = [
    { id: 'anthropic', name: 'Anthropic' },
    { id: 'openai', name: 'OpenAI' },
    { id: 'gemini', name: 'Google Gemini' },
  ]

  let settings = $state<EngineSettings | null>(null)
  let statuses = $state<Record<string, boolean>>({})
  let keyInputs = $state<Record<string, string>>({})
  let savingNote = $state<Record<string, string>>({})

  // New generic provider form.
  let newSlug = $state('')
  let newName = $state('')
  let newBaseUrl = $state('')
  let newModel = $state('')
  let newHeaders = $state('')

  // Training opt-in confirm gate.
  let confirmingOptIn = $state(false)

  const genericProviders = $derived(settings?.ai.generic_providers ?? [])
  const trainingOptIn = $derived(settings?.ai.training_opt_in ?? false)

  onMount(loadAll)

  async function loadAll() {
    try {
      settings = await loadEngineSettings()
    } catch {
      settings = null
    }
    await refreshStatuses()
  }

  async function refreshStatuses() {
    const ids = [...NATIVE.map((n) => n.id), ...genericProviders.map((g) => g.id)]
    const next: Record<string, boolean> = {}
    await Promise.all(
      ids.map(async (id) => {
        try {
          next[id] = await aiCheckKey(id)
        } catch {
          next[id] = false
        }
      })
    )
    statuses = next
  }

  async function saveKey(id: string) {
    const value = keyInputs[id] ?? ''
    if (!value.trim()) return
    try {
      await aiSetKey(id, value)
      keyInputs = { ...keyInputs, [id]: '' }
      savingNote = { ...savingNote, [id]: 'Saved' }
      await refreshStatuses()
      // Keep the panel's key status in sync if this is the active provider.
      if (id === $aiChat.activeProvider) {
        aiChat.update((s) => ({ ...s, hasKey: true }))
      }
    } catch (e) {
      savingNote = { ...savingNote, [id]: String(e) }
    }
    setTimeout(() => (savingNote = { ...savingNote, [id]: '' }), 2500)
  }

  async function removeKey(id: string) {
    try {
      await aiDeleteKey(id)
      await refreshStatuses()
      if (id === $aiChat.activeProvider) {
        aiChat.update((s) => ({ ...s, hasKey: false }))
      }
    } catch {
      /* ignore */
    }
  }

  function parseHeaders(text: string): Record<string, string> {
    const out: Record<string, string> = {}
    for (const line of text.split('\n')) {
      const idx = line.indexOf(':')
      if (idx > 0) {
        const k = line.slice(0, idx).trim()
        const v = line.slice(idx + 1).trim()
        if (k) out[k] = v
      }
    }
    return out
  }

  async function persistSettings(next: EngineSettings) {
    settings = next
    await saveEngineSettings(next)
    // Mirror generic providers into the panel store for the provider picker.
    aiChat.update((s) => ({ ...s, genericProviders: next.ai.generic_providers }))
  }

  async function addGeneric() {
    if (!settings) return
    const slug = newSlug.trim().replace(/^generic-/, '')
    if (!slug || !newBaseUrl.trim()) return
    const id = `generic-${slug}`
    if (settings.ai.generic_providers.some((g) => g.id === id)) return
    const provider: GenericProviderSetting = {
      id,
      display_name: newName.trim() || id,
      base_url: newBaseUrl.trim(),
      default_model: newModel.trim(),
      extra_headers: parseHeaders(newHeaders),
    }
    await persistSettings({
      ...settings,
      ai: { ...settings.ai, generic_providers: [...settings.ai.generic_providers, provider] },
    })
    newSlug = newName = newBaseUrl = newModel = newHeaders = ''
    await refreshStatuses()
  }

  async function removeGeneric(id: string) {
    if (!settings) return
    await persistSettings({
      ...settings,
      ai: {
        ...settings.ai,
        generic_providers: settings.ai.generic_providers.filter((g) => g.id !== id),
      },
    })
    await aiDeleteKey(id).catch(() => {})
    await refreshStatuses()
  }

  async function setTraining(on: boolean) {
    if (!settings) return
    if (on) {
      confirmingOptIn = true
      return
    }
    await persistSettings({ ...settings, ai: { ...settings.ai, training_opt_in: false } })
  }

  async function confirmTrainingOn() {
    if (!settings) return
    await persistSettings({ ...settings, ai: { ...settings.ai, training_opt_in: true } })
    confirmingOptIn = false
  }

  function headersToText(h: Record<string, string>): string {
    return Object.entries(h)
      .map(([k, v]) => `${k}: ${v}`)
      .join('\n')
  }
</script>

<section class="ai-settings">
  <div class="sec-title">AI — Provider Keys</div>
  <p class="hint">
    Keys are stored only in your OS keychain — never in settings or project files.
    Inputs are write-only; after saving, only stored/missing status is shown.
  </p>

  {#each NATIVE as p}
    <div class="key-row">
      <div class="key-label">
        {p.name}
        <span class="badge" class:ok={statuses[p.id]}>
          {statuses[p.id] ? 'Stored' : 'Missing'}
        </span>
      </div>
      <div class="key-controls">
        <input
          type="password"
          class="field"
          placeholder={statuses[p.id] ? 'Replace key…' : 'Paste API key…'}
          value={keyInputs[p.id] ?? ''}
          oninput={(e) => (keyInputs = { ...keyInputs, [p.id]: (e.target as HTMLInputElement).value })}
          autocomplete="off"
        />
        <button class="btn" onclick={() => saveKey(p.id)}>Save</button>
        {#if statuses[p.id]}
          <button class="btn ghost" onclick={() => removeKey(p.id)}>Remove</button>
        {/if}
      </div>
      {#if savingNote[p.id]}<span class="note">{savingNote[p.id]}</span>{/if}
    </div>
  {/each}

  <div class="sec-title sub">Generic OpenAI-compatible Providers</div>
  <p class="hint">
    For Groq, DeepSeek, Mistral, OpenRouter, Together, Ollama, LM Studio, etc.
    Each gets its own keychain entry under its id.
  </p>

  {#each genericProviders as g (g.id)}
    <div class="key-row">
      <div class="key-label">
        {g.display_name}
        <code class="gid">{g.id}</code>
        <span class="badge" class:ok={statuses[g.id]}>{statuses[g.id] ? 'Stored' : 'Missing'}</span>
      </div>
      <div class="muted-line">{g.base_url}{g.default_model ? ` · ${g.default_model}` : ''}</div>
      {#if Object.keys(g.extra_headers).length > 0}
        <div class="muted-line">headers: {headersToText(g.extra_headers).replace(/\n/g, ', ')}</div>
      {/if}
      <div class="key-controls">
        <input
          type="password"
          class="field"
          placeholder={statuses[g.id] ? 'Replace key…' : 'Paste API key…'}
          value={keyInputs[g.id] ?? ''}
          oninput={(e) => (keyInputs = { ...keyInputs, [g.id]: (e.target as HTMLInputElement).value })}
          autocomplete="off"
        />
        <button class="btn" onclick={() => saveKey(g.id)}>Save key</button>
        <button class="btn ghost" onclick={() => removeGeneric(g.id)} title="Remove provider">
          <Icon name="bin" size={13} />
        </button>
      </div>
    </div>
  {/each}

  <div class="add-generic">
    <div class="add-grid">
      <input class="field" placeholder="id slug (e.g. groq)" bind:value={newSlug} />
      <input class="field" placeholder="Display name" bind:value={newName} />
      <input class="field wide" placeholder="Base URL (e.g. https://api.groq.com/openai/v1)" bind:value={newBaseUrl} />
      <input class="field" placeholder="Default model" bind:value={newModel} />
    </div>
    <textarea
      class="field hdrs"
      rows="2"
      placeholder={'Extra headers, one per line:\nHTTP-Referer: https://...\nX-Title: GwenLand IDE'}
      bind:value={newHeaders}
    ></textarea>
    <button class="btn" onclick={addGeneric}>
      <Icon name="plus" size={13} /> Add provider
    </button>
  </div>

  <div class="sec-title sub">Privacy — Training Data</div>
  <p class="hint">
    Local chat history is always saved. This only controls whether future
    GwenLand core tooling may use your conversations as training data. Off by default.
  </p>
  {#if confirmingOptIn}
    <div class="confirm-box">
      <span>Allow local conversations to be used as training data by GwenLand core tools?</span>
      <div class="key-controls">
        <button class="btn" onclick={confirmTrainingOn}>Enable</button>
        <button class="btn ghost" onclick={() => (confirmingOptIn = false)}>Cancel</button>
      </div>
    </div>
  {:else}
    <Checkbox checked={trainingOptIn} onCheck={(v) => setTraining(v)}>
      Allow training use (global default for new conversations)
    </Checkbox>
  {/if}
</section>

<style>
  .ai-settings {
    display: flex;
    flex-direction: column;
  }
  .sec-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted-foreground);
    margin-bottom: 8px;
  }
  .sec-title.sub {
    margin-top: 22px;
  }
  .hint {
    font-size: 12px;
    color: var(--muted-foreground);
    margin: 0 0 12px;
    line-height: 1.45;
  }
  .key-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }
  .key-label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--foreground);
  }
  .gid {
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .muted-line {
    font-size: 11px;
    color: var(--muted-foreground);
    word-break: break-all;
  }
  .badge {
    font-size: 10px;
    font-weight: 700;
    padding: 1px 7px;
    border-radius: 999px;
    background-color: var(--secondary);
    color: var(--muted-foreground);
  }
  .badge.ok {
    background-color: color-mix(in srgb, #98c379 30%, var(--background));
    color: var(--foreground);
  }
  .key-controls {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .field {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 8px;
    font-size: 12px;
    color: var(--foreground);
    background-color: var(--input);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  textarea.field {
    height: auto;
    padding: 6px 8px;
    font-family: var(--font-mono);
    resize: vertical;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 12px;
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    color: var(--primary-foreground);
    background-color: var(--primary);
    border: 1px solid var(--primary);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .btn.ghost {
    color: var(--foreground);
    background-color: transparent;
    border-color: var(--border);
  }
  .note {
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .add-generic {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 0;
  }
  .add-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .add-grid .wide {
    grid-column: 1 / -1;
  }
  .hdrs {
    width: 100%;
    box-sizing: border-box;
  }
  .confirm-box {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    background-color: var(--secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: 12px;
  }
</style>
