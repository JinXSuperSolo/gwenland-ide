<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { get } from 'svelte/store'
  import {
    aiChat,
    toggleAiChat,
    setUnsentInput,
    clearError,
    setPendingTerminalError,
    clearPendingTerminalError,
  } from '../stores/ai-chat'
  import {
    initAiChat,
    sendMessage,
    cancelStream,
    createConversation,
    currentFileAttachment,
    currentSelectionAttachment,
  } from '../ai/ai-chat-setup'
  import { openSettings } from '../stores/ui'
  import { openFolder, workspace } from '../stores/workspace'
  import { openContextMenuSmart } from '../context-menu/globalContextMenu'
  import {
    aiErrorMessage,
    onAnyTerminalError,
    type ContextAttachment,
    type ImageAttachment,
  } from '../tauri/commands'
  import { assistantMode, MODE_META } from '../stores/assistant-mode'
  import { agentic } from '../stores/agentic'
  import { startAgentRun, cancelAgentSession } from '../agentic/agentic-setup'
  import AiConversation from './AiConversation.svelte'
  import AiMessage from './AiMessage.svelte'
  import AgentMessage from './agent/AgentMessage.svelte'
  import AgentTierMenu from './agent/AgentTierMenu.svelte'
  import AssistantModeMenu from './AssistantModeMenu.svelte'
  import ComposerModelMenu from './ComposerModelMenu.svelte'
  import DiffReviewPanel from './DiffReviewPanel.svelte'
  import Icon from './Icon.svelte'

  // Reactive: re-evaluates when a folder is opened/closed (workspace store),
  // unlike a one-shot get() which left the empty state stuck on "Open Folder".
  const hasProject = $derived($workspace.folderPath !== null)
  const streaming = $derived($aiChat.activeStreamId !== null)
  // The unified assistant mode (chat/agent/explain/plan) lives in a store so it
  // persists across panel collapse and reopen (M10 Wave 9).
  const isChatLike = $derived($assistantMode !== 'agent')
  const isAgent = $derived($assistantMode === 'agent')
  // Agent mode reuses the chat shell: the goal is typed in this composer and a
  // plan streams back. `agentStreaming` mirrors the chat `streaming` flag so the
  // send button flips to a stop control while the agent works.
  let agentGoal = $state('')
  const agentStreaming = $derived($agentic.activeStreamId !== null || $agentic.busy)
  const canSendAgent = $derived(
    hasProject && !!$aiChat.activeModel && agentGoal.trim().length > 0 && !agentStreaming
  )
  // Pending images for the next (multimodal) message.
  let images = $state<ImageAttachment[]>([])
  const canSend = $derived(
    $aiChat.hasKey &&
      !!$aiChat.activeModel &&
      !!$aiChat.activeConversationId &&
      !streaming &&
      ($aiChat.unsentInput.trim().length > 0 || images.length > 0)
  )

  let listEl = $state<HTMLDivElement | null>(null)
  let inputEl = $state<HTMLTextAreaElement | null>(null)
  let imageInputEl = $state<HTMLInputElement | null>(null)
  let dragOver = $state(false)

  // Suggestion chips for the workspace-open empty state (Requirement 3.9).
  const SUGGESTIONS = ['Explain this file', 'Find bugs', 'Write a test']

  /** Cap per image so a huge paste/drop can't bloat the request (~8 MB raw). */
  const MAX_IMAGE_BYTES = 8 * 1024 * 1024

  // Pending attachments for the next message (Requirement 14.9).
  let attachments = $state<ContextAttachment[]>([])
  let attachMenuOpen = $state(false)

  onMount(() => {
    void initAiChat()
    // Dev-only: drive the agent activity UI with mocked events (no provider).
    // Call `__gwenMockAgent()` / `__gwenMockAgentExhausted()` from the console.
    if (import.meta.env.DEV) {
      void import('../agentic/mock-activity').then((m) => {
        ;(window as unknown as Record<string, unknown>).__gwenMockAgent = m.runMockAgentActivity
        ;(window as unknown as Record<string, unknown>).__gwenMockAgentExhausted =
          m.runMockAgentExhausted
      })
    }
    // Terminal-error bridge: surface detected errors as an "explain" offer while
    // the panel is open (Requirement 15.1-15.2).
    let unlisten: (() => void) | null = null
    void onAnyTerminalError((err) => {
      if (get(aiChat).isOpen) setPendingTerminalError(err)
    }).then((fn) => (unlisten = fn))
    return () => unlisten?.()
  })

  // Auto-scroll to the newest content as it streams in.
  $effect(() => {
    // Touch the reactive deps so this re-runs on new tokens / messages, plus
    // live agent-run output (tool lines, reasoning, final answer).
    void $aiChat.messages.length
    void ($aiChat.messages.at(-1)?.content ?? '')
    void $agentic.toolLog.length
    void $agentic.streamedText
    void $agentic.toolFinal
    if (listEl) {
      tick().then(() => {
        if (listEl) listEl.scrollTop = listEl.scrollHeight
      })
    }
  })

  function onInput(e: Event) {
    setUnsentInput((e.currentTarget as HTMLTextAreaElement).value)
  }
  function onSend() {
    if (!canSend) return
    let text = $aiChat.unsentInput
    let atts = attachments
    // Edit mode: ask for a single unified-diff change (the existing diff proposal
    // flow renders it inline with Review). Auto-attach the current file/selection.
    if ($assistantMode === 'edit') {
      if (atts.length === 0) {
        const a = currentSelectionAttachment() ?? currentFileAttachment()
        if (a) atts = [a]
      }
      text =
        `Make the following change. Reply with a single unified diff (\`\`\`diff fenced) for the affected file(s) and a one-line summary — no extra prose:\n\n${text}`
    }
    void sendMessage(text, atts, images)
    attachments = []
    images = []
  }

  // --- Image upload (multimodal) ---------------------------------------------

  /** Read image files → base64 and add them as pending image attachments. */
  async function addImageFiles(files: Iterable<File>): Promise<void> {
    for (const file of files) {
      if (!file.type.startsWith('image/')) continue
      if (file.size > MAX_IMAGE_BYTES) continue
      const data = await fileToBase64(file)
      if (data) images = [...images, { mime: file.type, data }]
    }
  }

  /** Read a File into base64 (without the `data:...;base64,` prefix). */
  function fileToBase64(file: File): Promise<string | null> {
    return new Promise((resolve) => {
      const reader = new FileReader()
      reader.onload = () => {
        const result = String(reader.result)
        const comma = result.indexOf(',')
        resolve(comma >= 0 ? result.slice(comma + 1) : null)
      }
      reader.onerror = () => resolve(null)
      reader.readAsDataURL(file)
    })
  }

  function onPickImages(e: Event) {
    const input = e.currentTarget as HTMLInputElement
    if (input.files) void addImageFiles(Array.from(input.files))
    input.value = '' // allow re-picking the same file
    attachMenuOpen = false
  }
  function openImagePicker() {
    imageInputEl?.click()
  }
  function removeImage(i: number) {
    images = images.filter((_, idx) => idx !== i)
  }
  function onComposerPaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items
    if (!items) return
    const files: File[] = []
    for (const it of items) {
      if (it.kind === 'file' && it.type.startsWith('image/')) {
        const f = it.getAsFile()
        if (f) files.push(f)
      }
    }
    if (files.length) {
      e.preventDefault()
      void addImageFiles(files)
    }
  }
  function onComposerDrop(e: DragEvent) {
    dragOver = false
    const files = e.dataTransfer?.files
    if (files && files.length) {
      e.preventDefault()
      void addImageFiles(Array.from(files))
    }
  }
  // Agent mode: submit the message and run the inline tool loop directly.
  function onSendAgent() {
    if (!canSendAgent) return
    void startAgentRun(agentGoal)
    agentGoal = ''
  }
  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (isAgent) onSendAgent()
      else onSend()
    }
  }

  // Suggestion chip (Requirement 3.10): ensure a conversation exists so the
  // composer is enabled, then pre-fill it and focus the input.
  async function useSuggestion(text: string) {
    if (!$aiChat.activeConversationId) await createConversation()
    setUnsentInput(text)
    await tick()
    inputEl?.focus()
  }

  function attachFile() {
    const a = currentFileAttachment()
    if (a) attachments = [...attachments, a]
    attachMenuOpen = false
  }
  function attachSelection() {
    const a = currentSelectionAttachment()
    if (a) attachments = [...attachments, a]
    attachMenuOpen = false
  }
  function removeAttachment(i: number) {
    attachments = attachments.filter((_, idx) => idx !== i)
  }
  function attachTerminalError() {
    const err = $aiChat.pendingTerminalError
    if (!err) return
    attachments = [...attachments, { type: 'terminal_error', label: err.label, line: err.line }]
    if (!$aiChat.unsentInput.trim()) {
      setUnsentInput('Explain this terminal error and how to fix it.')
    }
    clearPendingTerminalError()
  }
  function attachmentLabel(a: ContextAttachment): string {
    if (a.type === 'file') return a.path.split(/[\\/]/).pop() || a.path
    if (a.type === 'selection') return `selection · ${a.path.split(/[\\/]/).pop() || a.path}`
    return `error: ${a.label}`
  }

  // Right-click in the AI panel opens its context menu (M9). Over the composer
  // text field it routes to the shared input menu (Cut/Copy/Paste); elsewhere it
  // shows the AI chat menu, carrying the current selection so Copy gates right.
  function onAiContextMenu(e: MouseEvent) {
    openContextMenuSmart(e, {
      scope: 'ai_chat',
      selectionText: window.getSelection()?.toString() || undefined,
    })
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<aside class="ai-panel" aria-label="AI Chat" oncontextmenu={onAiContextMenu}>
  <header class="ai-header">
    <span class="ai-title"><Icon name="sparks" size={14} /> AI</span>
    <div class="ai-header-actions">
      <!-- TODO(Wave 3): history button slot — wire when the conversation history view lands. -->
      <button
        class="icon-btn"
        title="New conversation"
        aria-label="New conversation"
        disabled={!hasProject || streaming || agentStreaming}
        onclick={createConversation}
      >
        <Icon name="plus" size={16} />
      </button>
      <button class="icon-btn" title="Settings" aria-label="Open settings" onclick={openSettings}>
        <Icon name="settings" size={16} />
      </button>
      <button class="icon-btn" title="Close" aria-label="Close AI panel" onclick={toggleAiChat}>
        <Icon name="xmark" size={16} />
      </button>
    </div>
  </header>

  <AiConversation />
  <DiffReviewPanel />

  <div class="ai-messages" bind:this={listEl}>
    {#if !hasProject}
      <div class="ai-empty">
        <span class="empty-icon"><Icon name="sparks" size={32} /></span>
        <p class="empty-headline">Open a folder to start</p>
        <p class="empty-subtext">GwenLand AI works best with a project open</p>
        <button class="empty-cta" onclick={() => void openFolder()}>Open Folder</button>
      </div>
    {:else if !$aiChat.activeConversationId || $aiChat.messages.length === 0}
      <div class="ai-empty">
        <span class="empty-icon"><Icon name="chat-bubble" size={32} /></span>
        <p class="empty-headline">{MODE_META[$assistantMode].hint}</p>
        {#if isChatLike}
          <div class="suggestions">
            {#each SUGGESTIONS as s}
              <button class="suggestion-chip" onclick={() => void useSuggestion(s)}>{s}</button>
            {/each}
          </div>
        {/if}
      </div>
    {:else}
      {#each $aiChat.messages as message (message.id)}
        {#if message.agent}
          <AgentMessage {message} />
        {:else}
          <AiMessage {message} />
        {/if}
      {/each}
    {/if}
  </div>

  <!-- Terminal-error explain offer (Requirement 15.3) — chat-like only -->
  {#if isChatLike && $aiChat.pendingTerminalError}
    <div class="ai-banner offer" role="status">
      <span class="offer-text">Terminal error: {$aiChat.pendingTerminalError.line}</span>
      <div class="offer-actions">
        <button class="offer-btn" onclick={attachTerminalError}>Explain</button>
        <button class="banner-x" aria-label="Dismiss" onclick={clearPendingTerminalError}>
          <Icon name="xmark" size={13} />
        </button>
      </div>
    </div>
  {/if}

  <!-- Provider/key/model banners apply to every mode (same provider + model). -->
  {#if $aiChat.lastError}
    <div class="ai-banner error" role="alert">
      <span>{aiErrorMessage($aiChat.lastError)}</span>
      <button class="banner-x" aria-label="Dismiss" onclick={clearError}>
        <Icon name="xmark" size={13} />
      </button>
    </div>
  {:else if hasProject && !$aiChat.hasKey}
    <div class="ai-banner warn">
      No API key for {$aiChat.activeProvider}. Add one in Settings → Provider Keys.
    </div>
  {:else if hasProject && $aiChat.hasKey && !$aiChat.activeModel}
    <div class="ai-banner warn">Select a model to send messages.</div>
  {/if}

  <div class="ai-composer">
    <div
      class="composer-surface"
      class:drag-over={dragOver}
      role="group"
      onpaste={onComposerPaste}
      ondragover={(e) => {
        if (e.dataTransfer?.types.includes('Files')) {
          e.preventDefault()
          dragOver = true
        }
      }}
      ondragleave={() => (dragOver = false)}
      ondrop={onComposerDrop}
    >
      <input
        type="file"
        accept="image/*"
        multiple
        bind:this={imageInputEl}
        onchange={onPickImages}
        style="display:none"
      />

      {#if isChatLike && attachments.length > 0}
        <div class="attach-chips">
          {#each attachments as a, i}
            <span class="attach-chip">
              <Icon name="attachment" size={11} />
              <span class="attach-name">{attachmentLabel(a)}</span>
              <button class="attach-x" aria-label="Remove attachment" onclick={() => removeAttachment(i)}>
                <Icon name="xmark" size={11} />
              </button>
            </span>
          {/each}
        </div>
      {/if}

      {#if isChatLike && images.length > 0}
        <div class="image-thumbs">
          {#each images as img, i}
            <div class="image-thumb">
              <img src={`data:${img.mime};base64,${img.data}`} alt="attachment" />
              <button class="image-x" aria-label="Remove image" onclick={() => removeImage(i)}>
                <Icon name="xmark" size={11} />
              </button>
            </div>
          {/each}
        </div>
      {/if}

      {#if isAgent}
        <textarea
          class="ai-input"
          rows="3"
          bind:this={inputEl}
          placeholder={hasProject
            ? "Describe a goal — I'll plan it first, then wait for approval"
            : 'Open a folder to use Agent mode…'}
          value={agentGoal}
          oninput={(e) => (agentGoal = (e.currentTarget as HTMLTextAreaElement).value)}
          onkeydown={onKeydown}
          disabled={!hasProject || agentStreaming}
        ></textarea>
      {:else}
        <textarea
          class="ai-input"
          rows="3"
          bind:this={inputEl}
          placeholder={$aiChat.activeConversationId
            ? `${MODE_META[$assistantMode].label}: Enter to send, Shift+Enter for newline`
            : `${MODE_META[$assistantMode].hint}…`}
          value={$aiChat.unsentInput}
          oninput={onInput}
          onkeydown={onKeydown}
          disabled={!$aiChat.activeConversationId}
        ></textarea>
      {/if}

      <div class="ai-composer-actions">
        {#if isChatLike}
          <div class="attach-wrap composer-attach">
            <button
              class="icon-btn"
              title="Attach context"
              aria-label="Attach context"
              disabled={!$aiChat.activeConversationId || streaming}
              onclick={() => (attachMenuOpen = !attachMenuOpen)}
            >
              <Icon name="attachment" size={15} />
            </button>
            {#if attachMenuOpen}
              <div class="attach-menu" role="menu">
                <button role="menuitem" onclick={attachFile}>Current file</button>
                <button role="menuitem" onclick={attachSelection}>Current selection</button>
                <button role="menuitem" onclick={openImagePicker}>Image…</button>
              </div>
            {/if}
          </div>
        {/if}

        <div class="composer-controls">
          <AssistantModeMenu placement="up" />
          <ComposerModelMenu placement="up" />
          {#if isAgent}
            <AgentTierMenu disabled={agentStreaming} />
          {/if}
        </div>

        {#if isAgent}
          {#if agentStreaming}
            <button class="send-arrow cancel" title="Stop" aria-label="Stop agent" onclick={() => void cancelAgentSession()}>
              <Icon name="xmark" size={16} />
            </button>
          {:else}
            <button class="send-arrow" disabled={!canSendAgent} onclick={onSendAgent} title="Start" aria-label="Start agent">
              <Icon name="arrow-up" size={16} />
            </button>
          {/if}
        {:else if streaming}
          <button class="send-arrow cancel" title="Stop" aria-label="Stop streaming" onclick={cancelStream}>
            <Icon name="xmark" size={16} />
          </button>
        {:else}
          <button
            class="send-arrow"
            disabled={!canSend}
            onclick={onSend}
            title="Send"
            aria-label="Send"
          >
            <Icon name="arrow-up" size={16} />
          </button>
        {/if}
      </div>
    </div>
  </div>
</aside>

<style>
  .ai-panel {
    /* ── M8 AI-scoped tokens ─────────────────────────────────────────
       Derived from the global theme tokens so the AI pane follows the
       active palette + accent picker. They inherit to every pane child
       (messages, conversation, thinking block, review panel). Diff
       add/remove colors stay semantic (green/red), not brand-tinted. */
    --ai-bg-base: var(--background);
    --ai-bg-surface: var(--card);
    --ai-bg-hover: rgba(255, 255, 255, 0.04);
    --ai-primary: var(--primary);
    --ai-primary-light: color-mix(in srgb, var(--primary) 80%, #ffffff);
    --ai-text-primary: var(--foreground);
    --ai-text-muted: var(--muted-foreground);
    /* Borderless chat pane: the subtle border token is transparent so every
       border/separator/outline that uses it disappears. Scrollbars keep their
       own colour via `--ai-scrollbar`. */
    --ai-border-subtle: transparent;
    --ai-scrollbar: color-mix(in srgb, var(--primary) 16%, transparent);
    --ai-thinking-bg: color-mix(in srgb, var(--primary) 7%, transparent);
    --ai-added-bg: rgba(40, 167, 69, 0.12);
    --ai-added-gutter: rgba(40, 167, 69, 0.25);
    --ai-removed-bg: rgba(220, 53, 69, 0.12);
    --ai-removed-gutter: rgba(220, 53, 69, 0.25);

    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
    background-color: var(--ai-bg-base);
    color: var(--ai-text-primary);
    font-family: var(--font-sans);
    overflow: hidden;
  }
  .ai-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 8px 0 16px;
    flex-shrink: 0;
  }
  .ai-title {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.03em;
    color: var(--ai-text-primary);
  }
  .ai-header-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .ai-messages {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    scrollbar-width: thin;
    scrollbar-color: var(--ai-scrollbar) transparent;
  }
  /* Flex children default to min-width:auto and won't shrink — let wide tool/
     diff content stay inside the pane instead of forcing horizontal overflow. */
  .ai-messages > :global(*) {
    min-width: 0;
    max-width: 100%;
  }
  /* Muted ~4px AI pane scrollbar (Req 2.8). */
  .ai-messages::-webkit-scrollbar {
    width: 4px;
  }
  .ai-messages::-webkit-scrollbar-track {
    background: transparent;
  }
  .ai-messages::-webkit-scrollbar-thumb {
    background-color: var(--ai-scrollbar);
    border-radius: 999px;
  }
  .ai-messages::-webkit-scrollbar-thumb:hover {
    background-color: color-mix(in srgb, var(--ai-primary) 30%, transparent);
  }
  .ai-empty {
    margin: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    color: var(--ai-text-muted);
    font-size: 12px;
    text-align: center;
    padding: 24px 16px;
  }
  .ai-empty p {
    margin: 0;
  }
  .empty-icon {
    display: inline-flex;
    margin-bottom: 2px;
    color: var(--ai-text-muted);
  }
  .empty-headline {
    font-size: 14px;
    font-weight: 600;
    color: color-mix(in srgb, var(--ai-text-primary) 80%, transparent);
  }
  .empty-subtext {
    font-size: 12px;
    color: var(--ai-text-muted);
    max-width: 240px;
  }
  /* Subtle primary-outline CTA (Req 3.6). */
  .empty-cta {
    margin-top: 4px;
    padding: 6px 16px;
    font-size: 12px;
    font-weight: 600;
    color: var(--ai-primary-light);
    background: transparent;
    border: 1px solid color-mix(in srgb, var(--ai-primary) 40%, transparent);
    border-radius: 999px;
    cursor: pointer;
    transition: background-color 0.12s ease, border-color 0.12s ease;
  }
  .empty-cta:hover {
    background-color: var(--ai-thinking-bg);
    border-color: var(--ai-primary);
  }
  .suggestions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px;
    margin-top: 6px;
  }
  /* Pill chips with --ai-border-subtle; primary hover (Req 3.11-3.12). */
  .suggestion-chip {
    padding: 5px 12px;
    font-size: 12px;
    color: var(--ai-text-muted);
    background: transparent;
    border: 1px solid var(--ai-border-subtle);
    border-radius: 999px;
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease, border-color 0.12s ease;
  }
  .suggestion-chip:hover {
    color: var(--ai-primary-light);
    background-color: var(--ai-thinking-bg);
    border-color: color-mix(in srgb, var(--ai-primary) 30%, transparent);
  }
  .ai-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0 16px 8px;
    padding: 7px 10px;
    font-size: 11.5px;
    border-radius: 10px;
    flex-shrink: 0;
  }
  .ai-banner.error {
    background-color: color-mix(in srgb, #e06c75 18%, var(--ai-bg-surface));
    color: var(--ai-text-primary);
  }
  .ai-banner.warn {
    background-color: var(--ai-bg-surface);
    color: var(--ai-text-muted);
  }
  .ai-banner.offer {
    background-color: var(--ai-thinking-bg);
    color: var(--ai-text-primary);
  }
  .offer-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .offer-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }
  .offer-btn {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 10px;
    color: var(--ai-bg-base);
    background-color: var(--ai-primary);
    border: none;
    border-radius: 999px;
    cursor: pointer;
  }
  .banner-x {
    display: inline-flex;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
    flex-shrink: 0;
  }
  .ai-composer {
    padding: 8px 16px 14px;
    flex-shrink: 0;
  }
  /* Floating, rounded composer surface (Req 2.9) — borderless/flat. */
  .composer-surface {
    background-color: var(--ai-bg-surface);
    border: none;
    border-radius: 16px;
    padding: 8px 8px 6px;
    box-shadow: var(--shadow-sm);
    transition: background-color 0.12s ease;
  }
  .composer-surface.drag-over {
    background-color: var(--ai-thinking-bg);
  }
  /* Pending image thumbnails (multimodal upload). */
  .image-thumbs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 6px;
  }
  .image-thumb {
    position: relative;
    width: 52px;
    height: 52px;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid var(--ai-border-subtle);
  }
  .image-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .image-x {
    position: absolute;
    top: 2px;
    right: 2px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border: none;
    border-radius: 50%;
    background-color: rgba(0, 0, 0, 0.6);
    color: #fff;
    cursor: pointer;
    padding: 0;
  }
  .ai-input {
    width: 100%;
    resize: none;
    font-family: var(--font-sans);
    font-size: 13px;
    line-height: 1.45;
    color: var(--ai-text-primary);
    background-color: transparent;
    border: none;
    padding: 4px 6px;
    box-sizing: border-box;
  }
  .ai-input::placeholder {
    color: var(--ai-text-muted);
  }
  .ai-input:focus {
    outline: none;
  }
  .ai-composer-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
  }
  /* Compact provider/model/reasoning pills live inside the composer toolbar. */
  .composer-controls {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .composer-attach {
    flex-shrink: 0;
  }
  .attach-wrap {
    position: relative;
  }
  .attach-menu {
    position: absolute;
    bottom: 30px;
    left: 0;
    z-index: 20;
    min-width: 150px;
    background-color: var(--ai-bg-surface);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 12px;
    box-shadow: var(--shadow-lg, 0 4px 14px rgba(0, 0, 0, 0.4));
    overflow: hidden;
  }
  .attach-menu button {
    display: block;
    width: 100%;
    text-align: left;
    padding: 7px 10px;
    font-size: 12px;
    color: var(--ai-text-primary);
    background: transparent;
    border: none;
    cursor: pointer;
  }
  .attach-menu button:hover {
    background-color: var(--ai-bg-hover);
  }
  .attach-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }
  .attach-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 100%;
    padding: 2px 4px 2px 7px;
    font-size: 11px;
    background-color: var(--ai-bg-hover);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 999px;
    color: var(--ai-text-primary);
  }
  .attach-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 180px;
  }
  .attach-x {
    display: inline-flex;
    border: none;
    background: transparent;
    color: var(--ai-text-muted);
    cursor: pointer;
    padding: 0;
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  /* Rounded arrow-style send control (Req 2.10). */
  .send-arrow {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    color: var(--ai-bg-base);
    background-color: var(--ai-primary);
    border: none;
    border-radius: 999px;
    cursor: pointer;
    transition: background-color 0.12s ease, opacity 0.12s ease;
  }
  .send-arrow:hover:not(:disabled) {
    background-color: var(--ai-primary-light);
  }
  .send-arrow:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .send-arrow.cancel {
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
  }
  .send-arrow.cancel:hover:not(:disabled) {
    background-color: rgba(255, 255, 255, 0.08);
  }
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
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
</style>
