<script lang="ts">
  import { aboutOpen, closeAbout } from '../stores/ui'
  import Icon from './Icon.svelte'

  // Enter is a quick "dismiss" affordance for this dialog. Escape is owned by the
  // centralized overlay stack (App.svelte), so it's intentionally not handled here
  // — otherwise a single Escape would close this AND the next overlay below it.
  function onKeydown(e: KeyboardEvent) {
    if (!$aboutOpen) return
    if (e.key === 'Enter') {
      e.preventDefault()
      closeAbout()
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if $aboutOpen}
  <div
    class="overlay gw-anim-overlay"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) closeAbout() }}
  >
    <div class="dialog gw-anim-pop" role="alertdialog" aria-modal="true" aria-label="About GwenLand IDE">
      <div class="about-hero">
        <Icon name="code" size={48} />
      </div>
      <div class="about-content">
        <div class="about-title">GwenLand IDE</div>
        <div class="about-version">Version 0.1.0</div>
        <div class="about-tagline">Zero Bloat. Pure Rust.</div>
      </div>
      <div class="dialog-actions">
        <button type="button" class="btn" onclick={closeAbout}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    justify-content: center;
    align-items: center;
    background-color: rgba(0, 0, 0, 0.4);
  }
  .dialog {
    width: 320px;
    max-width: 90vw;
    background-color: var(--popover);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-xl);
    padding: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }
  .about-hero {
    color: var(--primary);
  }
  .about-content {
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .about-title {
    font-size: 16px;
    font-weight: 700;
    color: var(--foreground);
    letter-spacing: 0.02em;
  }
  .about-version {
    font-size: 12px;
    color: var(--muted-foreground);
    font-family: var(--font-mono);
  }
  .about-tagline {
    font-size: 13px;
    color: var(--muted-foreground);
    margin-top: 4px;
  }
  .dialog-actions {
    margin-top: 8px;
    width: 100%;
    display: flex;
    justify-content: center;
  }
  .btn {
    padding: 7px 24px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background-color: var(--secondary);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
    transition: background-color 0.12s ease;
  }
  .btn:hover {
    background-color: var(--hover-bg);
  }
</style>
