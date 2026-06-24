<script lang="ts">
  import { persona, savePersona, DEFAULT_SYSTEM_PROMPT } from '../stores/workspace-persona'

  /**
   * Inline `/system` editor (GWEN-334): edit the workspace system prompt in a
   * textarea and Save/Cancel. Floats above the composer like the other inline
   * pickers. Saving writes through to `.gwenland/GwenLand.md`; an empty save is
   * equivalent to `/reset-system` (the engine default applies).
   */
  let { onClose }: { onClose: () => void } = $props()

  // Seed with the current custom prompt; if none, show the default as a starting
  // point the user can edit (placeholder communicates the fallback either way).
  let draft = $state($persona.systemPrompt || DEFAULT_SYSTEM_PROMPT)
  let saving = $state(false)
  let error = $state('')

  async function save() {
    saving = true
    error = ''
    try {
      // Saving the default verbatim is treated as "no custom prompt" so the file
      // stays clean and the engine default keeps applying.
      const trimmed = draft.trim()
      const systemPrompt = trimmed === DEFAULT_SYSTEM_PROMPT.trim() ? '' : trimmed
      await savePersona({ ...$persona, systemPrompt })
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

<div class="sys-editor" role="dialog" aria-label="Edit system prompt" tabindex="-1" onkeydown={onKeydown}>
  <div class="sys-title">System Prompt</div>
  <!-- svelte-ignore a11y_autofocus -->
  <textarea
    class="sys-textarea"
    bind:value={draft}
    rows="8"
    placeholder={DEFAULT_SYSTEM_PROMPT}
    autofocus
    spellcheck="false"
  ></textarea>
  <div class="sys-hint">Leave blank (or unchanged) to use the GwenLand default. ⌘/Ctrl+Enter saves.</div>
  {#if error}<div class="sys-error">{error}</div>{/if}
  <div class="sys-actions">
    <button type="button" class="sys-btn ghost" onclick={onClose} disabled={saving}>Cancel</button>
    <button type="button" class="sys-btn primary" onclick={save} disabled={saving}>
      {saving ? 'Saving…' : 'Save'}
    </button>
  </div>
</div>

<style>
  .sys-editor {
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
  .sys-title {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--ai-text-muted);
  }
  .sys-textarea {
    width: 100%;
    resize: vertical;
    min-height: 120px;
    padding: 8px 9px;
    font-family: var(--font-mono, monospace);
    font-size: 12px;
    line-height: 1.45;
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 8px;
    box-sizing: border-box;
  }
  .sys-textarea:focus {
    outline: none;
    border-color: color-mix(in srgb, var(--ai-primary) 35%, transparent);
  }
  .sys-textarea::placeholder {
    color: var(--ai-text-muted);
  }
  .sys-hint {
    font-size: 11px;
    color: var(--ai-text-muted);
  }
  .sys-error {
    font-size: 11px;
    color: #e06c75;
  }
  .sys-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .sys-btn {
    height: 26px;
    padding: 0 12px;
    font-size: 12px;
    font-weight: 600;
    border-radius: 999px;
    border: none;
    cursor: pointer;
  }
  .sys-btn.ghost {
    background: transparent;
    color: var(--ai-text-muted);
  }
  .sys-btn.ghost:hover:not(:disabled) {
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
  }
  .sys-btn.primary {
    background-color: var(--ai-primary);
    color: var(--ai-bg-base);
  }
  .sys-btn.primary:hover:not(:disabled) {
    background-color: var(--ai-primary-light);
  }
  .sys-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
