<script lang="ts">
  import { aiChat } from '../stores/ai-chat'
  import {
    selectConversation,
    createConversation,
    renameConversation,
    deleteConversation,
  } from '../ai/ai-chat-setup'
  import { workspace } from '../stores/workspace'
  import Icon from './Icon.svelte'

  // GWEN-324: slide-out conversation history, replacing the old inline dropdown.
  // Opened by the clock/history button in the AI panel header.
  let { open = false, onClose }: { open?: boolean; onClose: () => void } = $props()

  const hasProject = $derived($workspace.folderPath !== null)
  const streaming = $derived($aiChat.activeStreamId !== null)

  let renamingId = $state<string | null>(null)
  let renameValue = $state('')

  function startRename(id: string, title: string) {
    renamingId = id
    renameValue = title
  }
  async function confirmRename() {
    if (renamingId && renameValue.trim()) {
      await renameConversation(renamingId, renameValue.trim())
    }
    renamingId = null
  }
  async function onPick(id: string) {
    if (streaming) return
    await selectConversation(id)
    onClose()
  }
  async function onNew() {
    if (!hasProject || streaming) return
    await createConversation()
    onClose()
  }
  async function onDelete(id: string, e: MouseEvent) {
    e.stopPropagation()
    await deleteConversation(id)
  }
</script>

{#if open}
  <!-- Scrim closes the slide-out on outside click. -->
  <div class="hist-scrim" role="presentation" onclick={onClose}></div>
  <aside class="hist-panel" aria-label="Conversation history">
    <header class="hist-header">
      <span class="hist-title">History</span>
      <div class="hist-actions">
        <button
          class="icon-btn"
          title="New conversation"
          aria-label="New conversation"
          disabled={!hasProject || streaming}
          onclick={onNew}
        >
          <Icon name="plus" size={15} />
        </button>
        <button class="icon-btn" title="Close" aria-label="Close history" onclick={onClose}>
          <Icon name="xmark" size={15} />
        </button>
      </div>
    </header>

    <div class="hist-list">
      {#if $aiChat.conversations.length === 0}
        <div class="hist-empty">No conversations yet.</div>
      {:else}
        {#each $aiChat.conversations as conv (conv.id)}
          {#if renamingId === conv.id}
            <div class="hist-rename">
              <input
                class="hist-rename-input"
                bind:value={renameValue}
                onkeydown={(e) => {
                  if (e.key === 'Enter') confirmRename()
                  else if (e.key === 'Escape') (renamingId = null)
                }}
                aria-label="Conversation title"
              />
              <button class="icon-btn" title="Save" aria-label="Save title" onclick={confirmRename}>
                <Icon name="check" size={14} />
              </button>
            </div>
          {:else}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="hist-item"
              class:active={conv.id === $aiChat.activeConversationId}
              role="button"
              tabindex="0"
              onclick={() => onPick(conv.id)}
              onkeydown={(e) => e.key === 'Enter' && onPick(conv.id)}
            >
              <Icon name="chat-bubble" size={13} class="hi-icon" />
              <span class="hi-title">{conv.title}</span>
              <button
                class="hi-act"
                title="Rename"
                aria-label="Rename conversation"
                onclick={(e) => {
                  e.stopPropagation()
                  startRename(conv.id, conv.title)
                }}
              >
                <Icon name="edit-pencil" size={13} />
              </button>
              <button
                class="hi-act danger"
                title="Delete"
                aria-label="Delete conversation"
                onclick={(e) => onDelete(conv.id, e)}
              >
                <Icon name="bin" size={13} />
              </button>
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  </aside>
{/if}

<style>
  .hist-scrim {
    position: absolute;
    inset: 0;
    z-index: 30;
    background-color: rgba(0, 0, 0, 0.25);
  }
  .hist-panel {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 31;
    width: 260px;
    max-width: 85%;
    display: flex;
    flex-direction: column;
    background-color: var(--ai-bg-surface);
    border-left: 1px solid var(--border);
    box-shadow: -8px 0 24px rgba(0, 0, 0, 0.35);
    animation: hist-slide 0.16s ease-out;
  }
  @keyframes hist-slide {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }
  @media (prefers-reduced-motion: reduce) {
    .hist-panel { animation: none; }
  }
  .hist-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 8px 10px 14px;
    flex-shrink: 0;
  }
  .hist-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--ai-text-muted);
  }
  .hist-actions {
    display: flex;
    gap: 2px;
  }
  .hist-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 6px 12px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .hist-empty {
    font-size: 12px;
    color: var(--ai-text-muted);
    padding: 8px;
  }
  .hist-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 8px;
    cursor: pointer;
    color: var(--ai-text-primary);
  }
  .hist-item:hover {
    background-color: var(--ai-bg-hover);
  }
  .hist-item.active {
    background-color: var(--ai-thinking-bg);
  }
  .hist-item :global(.hi-icon) {
    color: var(--ai-text-muted);
    flex-shrink: 0;
  }
  .hi-title {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hi-act {
    opacity: 0;
    pointer-events: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--ai-text-muted);
    cursor: pointer;
    flex-shrink: 0;
  }
  .hist-item:hover .hi-act {
    opacity: 1;
    pointer-events: auto;
  }
  .hi-act:hover {
    color: var(--ai-text-primary);
    background-color: rgba(255, 255, 255, 0.06);
  }
  .hi-act.danger:hover {
    color: var(--destructive, #e06c75);
  }
  .hist-rename {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
  }
  .hist-rename-input {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 8px;
    font-size: 13px;
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-base);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .hist-rename-input:focus {
    outline: none;
    border-color: var(--primary);
  }
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--ai-text-muted);
    cursor: pointer;
  }
  .icon-btn:hover:not(:disabled) {
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
