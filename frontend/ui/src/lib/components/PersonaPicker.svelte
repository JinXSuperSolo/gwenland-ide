<script lang="ts">
  import { persona, TONE_PRESETS, savePersona, type Tone } from '../stores/workspace-persona'
  import Icon from './Icon.svelte'

  /**
   * Inline `/persona` picker (GWEN-334): edit the AI name + pick a tone preset,
   * then save to `.gwenland/GwenLand.md`. Floats above the composer like the
   * `/model` and `/mode` pickers. Self-contained — reads the persona store,
   * writes via `savePersona`, and calls `onClose` when done/cancelled.
   */
  let { onClose }: { onClose: () => void } = $props()

  // Local draft seeded from the current config; committed only on Save.
  let name = $state($persona.persona.name)
  let tone = $state<Tone>($persona.persona.tone)
  let saving = $state(false)
  let error = $state('')

  async function save() {
    saving = true
    error = ''
    try {
      await savePersona({
        ...$persona,
        persona: { ...$persona.persona, name: name.trim() || 'GwenLand AI', tone },
      })
      onClose()
    } catch (e) {
      error = String(e)
    } finally {
      saving = false
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      onClose()
    } else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      void save()
    }
  }
</script>

<div class="persona-picker" role="dialog" aria-label="Edit persona" tabindex="-1" onkeydown={onKeydown}>
  <div class="pp-title">AI Persona</div>

  <label class="pp-field">
    <span class="pp-label">Name</span>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      class="pp-input"
      bind:value={name}
      placeholder="GwenLand AI"
      autofocus
      spellcheck="false"
    />
  </label>

  <div class="pp-label">Tone</div>
  <div class="pp-tones">
    {#each TONE_PRESETS as t (t.id)}
      <button
        type="button"
        class="pp-tone"
        class:active={t.id === tone}
        title={t.hint}
        onclick={() => (tone = t.id)}
      >
        <span class="pp-tone-label">{t.label}</span>
        <span class="pp-tone-hint">{t.hint}</span>
        {#if t.id === tone}<span class="pp-tone-check"><Icon name="check" size={13} /></span>{/if}
      </button>
    {/each}
  </div>

  {#if error}<div class="pp-error">{error}</div>{/if}

  <div class="pp-actions">
    <button type="button" class="pp-btn ghost" onclick={onClose} disabled={saving}>Cancel</button>
    <button type="button" class="pp-btn primary" onclick={save} disabled={saving}>
      {saving ? 'Saving…' : 'Save'}
    </button>
  </div>
</div>

<style>
  .persona-picker {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(100% + 6px);
    z-index: 40;
    padding: 10px;
    background-color: var(--ai-bg-surface);
    border: 1px solid var(--ai-border-subtle, transparent);
    border-radius: 12px;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.45));
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .pp-title {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--ai-text-muted);
  }
  .pp-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .pp-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--ai-text-muted);
  }
  .pp-input {
    height: 28px;
    padding: 0 9px;
    font-family: var(--font-sans);
    font-size: 13px;
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 8px;
  }
  .pp-input:focus {
    outline: none;
    border-color: color-mix(in srgb, var(--ai-primary) 35%, transparent);
  }
  .pp-tones {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .pp-tone {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    color: var(--ai-text-primary);
  }
  .pp-tone:hover {
    background-color: var(--ai-bg-hover);
  }
  .pp-tone.active {
    background-color: color-mix(in srgb, var(--ai-primary) 14%, transparent);
  }
  .pp-tone-label {
    font-size: 12px;
    font-weight: 600;
    flex-shrink: 0;
  }
  .pp-tone-hint {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: var(--ai-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pp-tone-check {
    display: inline-flex;
    color: var(--ai-primary-light);
    flex-shrink: 0;
  }
  .pp-error {
    font-size: 11px;
    color: #e06c75;
  }
  .pp-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .pp-btn {
    height: 26px;
    padding: 0 12px;
    font-size: 12px;
    font-weight: 600;
    border-radius: 999px;
    border: none;
    cursor: pointer;
  }
  .pp-btn.ghost {
    background: transparent;
    color: var(--ai-text-muted);
  }
  .pp-btn.ghost:hover:not(:disabled) {
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
  }
  .pp-btn.primary {
    background-color: var(--ai-primary);
    color: var(--ai-bg-base);
  }
  .pp-btn.primary:hover:not(:disabled) {
    background-color: var(--ai-primary-light);
  }
  .pp-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
