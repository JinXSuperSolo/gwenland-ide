<script lang="ts">
  import { aiChat } from '../stores/ai-chat'
  import {
    createConversation,
    selectConversation,
    renameConversation,
    deleteConversation,
    setTrainingOptIn,
  } from '../ai/ai-chat-setup'
  import { workspace } from '../stores/workspace'
  import Icon from './Icon.svelte'
  import Dropdown, { type DropdownOption } from './Dropdown.svelte'
  import Checkbox from './Checkbox.svelte'

  /**
   * Conversation controls (Requirement 12.3): a compact picker plus new /
   * rename / delete. Rename and delete use inline confirm states rather than
   * native dialogs (which the webview may suppress). The picker is the shared
   * custom Dropdown — no native select element in the AI pane (Requirement 4.1).
   */

  let renaming = $state(false)
  let renameValue = $state('')
  let confirmingDelete = $state(false)

  const active = $derived(
    $aiChat.conversations.find((c) => c.id === $aiChat.activeConversationId) ?? null
  )
  const hasProject = $derived($workspace.folderPath !== null)
  const streaming = $derived($aiChat.activeStreamId !== null)

  const convOptions = $derived<DropdownOption[]>(
    $aiChat.conversations.map((c) => ({ id: c.id, label: c.title }))
  )

  function startRename() {
    if (!active) return
    renameValue = active.title
    renaming = true
    confirmingDelete = false
  }
  async function confirmRename() {
    const id = active?.id
    if (id && renameValue.trim()) await renameConversation(id, renameValue.trim())
    renaming = false
  }
  async function onDelete() {
    const id = active?.id
    if (id) await deleteConversation(id)
    confirmingDelete = false
  }
  function onToggleTraining(checked: boolean) {
    if (active) void setTrainingOptIn(active.id, checked)
  }
</script>

<div class="conv">
  {#if renaming}
    <input
      class="conv-input"
      bind:value={renameValue}
      onkeydown={(e) => {
        if (e.key === 'Enter') confirmRename()
        else if (e.key === 'Escape') (renaming = false)
      }}
      aria-label="Conversation title"
    />
    <button class="icon-btn" title="Save" aria-label="Save title" onclick={confirmRename}>
      <Icon name="nav-arrow-right" size={14} />
    </button>
    <button class="icon-btn" title="Cancel" aria-label="Cancel rename" onclick={() => (renaming = false)}>
      <Icon name="xmark" size={14} />
    </button>
  {:else if confirmingDelete}
    <span class="conv-confirm">Delete this conversation?</span>
    <button class="icon-btn danger" title="Confirm delete" aria-label="Confirm delete" onclick={onDelete}>
      <Icon name="bin" size={14} />
    </button>
    <button class="icon-btn" title="Cancel" aria-label="Cancel delete" onclick={() => (confirmingDelete = false)}>
      <Icon name="xmark" size={14} />
    </button>
  {:else}
    <Dropdown
      options={convOptions}
      value={$aiChat.activeConversationId ?? ''}
      onSelect={(id) => void selectConversation(id)}
      label="Select conversation"
      placeholder="No conversations"
      disabled={$aiChat.conversations.length === 0 || streaming}
    />
    <button
      class="icon-btn"
      title="New conversation"
      aria-label="New conversation"
      disabled={!hasProject || streaming}
      onclick={createConversation}
    >
      <Icon name="plus" size={14} />
    </button>
    <button
      class="icon-btn"
      title="Rename"
      aria-label="Rename conversation"
      disabled={!active || streaming}
      onclick={startRename}
    >
      <Icon name="edit-pencil" size={14} />
    </button>
    <button
      class="icon-btn"
      title="Delete"
      aria-label="Delete conversation"
      disabled={!active || streaming}
      onclick={() => (confirmingDelete = true)}
    >
      <Icon name="bin" size={14} />
    </button>
  {/if}
</div>

{#if active && !renaming && !confirmingDelete}
  <div class="train-row">
    <Checkbox
      checked={active.training_opt_in}
      onCheck={onToggleTraining}
      title="Allow this conversation to be used as training data by GwenLand core tools"
    >
      Allow training use for this conversation
    </Checkbox>
  </div>
{/if}

<style>
  .conv {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 16px 6px;
  }
  .conv-input {
    flex: 1;
    min-width: 0;
    height: 26px;
    padding: 0 10px;
    font-size: 12px;
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-surface);
    border: 1px solid transparent;
    border-radius: 999px;
    transition: border-color 0.12s ease;
  }
  .conv-input:focus {
    outline: none;
    border-color: var(--ai-border-subtle);
  }
  .conv-confirm {
    flex: 1;
    font-size: 12px;
    color: var(--ai-text-muted);
  }
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--ai-text-muted);
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease;
  }
  .icon-btn:hover:not(:disabled) {
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
  }
  .icon-btn.danger:hover:not(:disabled) {
    color: var(--destructive, #e06c75);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .train-row {
    display: flex;
    align-items: center;
    padding: 4px 16px 6px;
  }
</style>
