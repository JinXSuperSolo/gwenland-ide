<script lang="ts">
  import {
    diffReview,
    reviewSummary,
    acceptAll,
    rejectAll,
    cancelReview,
    acceptHunk,
    rejectHunk,
    moveActiveHunk,
    activeHunk,
  } from '../stores/diff-review'
  import { revealLine } from '../editor/active-editor'
  import Icon from './Icon.svelte'

  /**
   * Floating editor action bar for an active review (Requirement 14.1-14.4) and
   * host for the review keyboard shortcuts (Requirement 12.7-12.11, 15.3).
   * Renders nothing unless a review is active, so App can mount it unconditionally.
   */

  function focusedInTextInput(): boolean {
    const el = document.activeElement as HTMLElement | null
    if (!el) return false
    const tag = el.tagName
    // Block the composer/model inputs (Req 15.3) but allow the keys inside the
    // CodeMirror editor, where reviewing happens (it is contenteditable).
    if (tag === 'INPUT' || tag === 'TEXTAREA') return true
    if (el.isContentEditable && !el.closest('.cm-editor')) return true
    return false
  }

  function onKeydown(e: KeyboardEvent) {
    if (!$diffReview.active) return
    // Never steal keys while typing in the composer, model input, or the editor
    // itself (the CodeMirror content is contenteditable) — Requirement 15.3.
    if (focusedInTextInput()) return
    switch (e.key) {
      case ']': {
        e.preventDefault()
        const h = moveActiveHunk(1)
        if (h) revealLine(h.oldStart)
        break
      }
      case '[': {
        e.preventDefault()
        const h = moveActiveHunk(-1)
        if (h) revealLine(h.oldStart)
        break
      }
      case 'a': {
        e.preventDefault()
        const h = activeHunk()
        if (h && h.status === 'pending') acceptHunk(h.id)
        break
      }
      case 'r': {
        e.preventDefault()
        const h = activeHunk()
        if (h && h.status === 'pending') rejectHunk(h.id)
        break
      }
      case 'Escape':
        e.preventDefault()
        cancelReview()
        break
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if $diffReview.active}
  <div class="diff-action-bar" role="region" aria-label="Review changes">
    <div class="dab-info">
      <span class="dab-title"><Icon name="page-plus" size={13} /> Review Changes</span>
      <span class="dab-counts">
        {$reviewSummary.files} {$reviewSummary.files === 1 ? 'file' : 'files'} ·
        {$reviewSummary.hunks} {$reviewSummary.hunks === 1 ? 'hunk' : 'hunks'} ·
        <span class="dab-add">+{$reviewSummary.added}</span>
        <span class="dab-rem">−{$reviewSummary.removed}</span>
        {#if $reviewSummary.failed > 0}<span class="dab-fail">· {$reviewSummary.failed} failed</span>{/if}
      </span>
    </div>
    <div class="dab-actions">
      <button class="dab-btn accept" onclick={acceptAll}>Accept All</button>
      <button class="dab-btn reject" onclick={rejectAll}>Reject All</button>
      <button class="dab-btn cancel" onclick={cancelReview}>Cancel</button>
    </div>
  </div>
{/if}

<style>
  .diff-action-bar {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 40;
    display: flex;
    align-items: center;
    gap: 16px;
    max-width: calc(100% - 24px);
    padding: 6px 8px 6px 12px;
    background-color: var(--card);
    border: 1px solid color-mix(in srgb, var(--primary) 30%, transparent);
    border-radius: 999px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    font-family: var(--font-sans);
  }
  .dab-info {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .dab-title {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
    color: var(--foreground);
    white-space: nowrap;
  }
  .dab-counts {
    font-size: 11px;
    color: var(--muted-foreground);
    white-space: nowrap;
  }
  .dab-add {
    color: #5fb572;
  }
  .dab-rem {
    color: #e0707c;
  }
  .dab-fail {
    color: #e0707c;
  }
  .dab-actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .dab-btn {
    font-size: 11.5px;
    font-weight: 600;
    padding: 4px 12px;
    border-radius: 999px;
    border: 1px solid transparent;
    cursor: pointer;
  }
  .dab-btn.accept {
    color: #fff;
    background-color: rgba(40, 167, 69, 0.85);
  }
  .dab-btn.reject {
    color: var(--foreground);
    background-color: rgba(255, 255, 255, 0.06);
  }
  .dab-btn.cancel {
    color: var(--muted-foreground);
    background-color: transparent;
    border-color: rgba(255, 255, 255, 0.12);
  }
  .dab-btn:hover {
    filter: brightness(1.1);
  }
</style>
