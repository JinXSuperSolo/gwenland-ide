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
    setTrainingOptIn,
    currentFileAttachment,
    currentSelectionAttachment,
  } from '../ai/ai-chat-setup'
  import { openSettings } from '../stores/ui'
  import { workspace } from '../stores/workspace'
  import { openContextMenuSmart } from '../context-menu/globalContextMenu'
  import {
    aiErrorMessage,
    onAnyTerminalError,
    type ContextAttachment,
    type ImageAttachment,
  } from '../tauri/commands'
  import { assistantMode } from '../stores/assistant-mode'
  import { agentic } from '../stores/agentic'
  import { startAgentRun, cancelAgentSession } from '../agentic/agentic-setup'
  import AiHistory from './AiHistory.svelte'
  import AiMessage from './AiMessage.svelte'
  import SlashCommandMenu from './SlashCommandMenu.svelte'
  import Checkbox from './Checkbox.svelte'
  import AgentMessage from './agent/AgentMessage.svelte'
  import ComposerActionsMenu from './ComposerActionsMenu.svelte'
  import ComposerModelMenu from './ComposerModelMenu.svelte'
  import ReasoningMenu from './ReasoningMenu.svelte'
  import DiffReviewPanel from './DiffReviewPanel.svelte'
  import Icon from './Icon.svelte'
  import {
    parseSlashQuery,
    filterCommands,
    exactCommand,
    type SlashCommand,
  } from '../stores/slash-commands'
  import { runSlashCommand } from '../ai/slash-command-setup'
  import { setModel } from '../ai/ai-chat-setup'
  import { changeAgentTier } from '../agentic/agentic-setup'
  import type { AgentTier } from '../tauri/commands'
  import MentionMenu from './MentionMenu.svelte'
  import MentionPill from './MentionPill.svelte'
  import {
    SPECIAL_PROVIDERS,
    parseMentionQuery,
    fuzzySearch,
    parseLineRange,
    type MentionCandidate,
    type MentionItem,
  } from '../stores/mention-providers'
  import { getWorkspaceIndex, resolveAllMentions } from '../ai/mention-setup'
  import PersonaPicker from './PersonaPicker.svelte'
  import SystemPromptEditor from './SystemPromptEditor.svelte'
  import { persona, loadPersona, resetPersona } from '../stores/workspace-persona'

  // Reactive: re-evaluates when a folder is opened/closed. Agent tools and
  // conversation creation need a project, so sends gate on this silently (no
  // banner or empty state is shown when there's no workspace).
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
  // GWEN-324: no longer requires an existing conversation — sendMessage creates
  // one on first send. Still requires a project (conversations are per-project),
  // a key, and a model.
  const canSend = $derived(
    hasProject &&
      $aiChat.hasKey &&
      !!$aiChat.activeModel &&
      !streaming &&
      ($aiChat.unsentInput.trim().length > 0 || images.length > 0)
  )

  // --- Slash commands (GWEN-333) --------------------------------------------
  let slashMenuDismissed = $state(false)
  let slashActiveIndex = $state(0)
  // Inline pickers opened by /model, /mode, /persona, /system.
  let modelPickerOpen = $state(false)
  let modePickerOpen = $state(false)
  let personaPickerOpen = $state(false)
  let systemEditorOpen = $state(false)
  // The composer text is the agent goal in agent mode, else the chat input.
  const composerText = $derived(isAgent ? agentGoal : $aiChat.unsentInput)
  // Parse the current text into a slash query; null = not a slash invocation.
  const slashQuery = $derived(parseSlashQuery(composerText))
  // Show the autocomplete while typing the command name (before any space).
  const slashCommands = $derived<SlashCommand[]>(
    slashQuery && !slashQuery.hasArgs ? filterCommands(slashQuery.token) : []
  )
  const slashOpen = $derived(slashMenuDismissed ? false : slashCommands.length > 0)

  const MODE_TIERS: { id: AgentTier; label: string; hint: string }[] = [
    { id: 'ask', label: 'Ask', hint: 'Approve every edit & command' },
    { id: 'accept_for_me', label: 'Accept for Me', hint: 'Auto-approve small, safe changes' },
    { id: 'full_control', label: 'Full Control', hint: 'Run autonomously; stop at destructive steps' },
  ]

  // Reset the highlighted row + un-dismiss whenever the filtered set changes.
  $effect(() => {
    void slashCommands.length
    if (slashActiveIndex >= slashCommands.length) slashActiveIndex = 0
  })
  // Re-arm the menu when the user clears the line (so retyping `/` reopens it).
  $effect(() => {
    if (!composerText.startsWith('/')) slashMenuDismissed = false
  })

  function setComposerText(text: string): void {
    if (isAgent) agentGoal = text
    else setUnsentInput(text)
  }

  // --- @ mentions (GWEN-332) -------------------------------------------------
  // Resolved mentions attached to the next message (rendered as pills). One
  // shared list: only one composer (chat or agent) is active at a time.
  let mentions = $state<MentionItem[]>([])
  let mentionCandidates = $state<MentionCandidate[]>([])
  let mentionActiveIndex = $state(0)
  // The live `@` query under the caret (drives the dropdown), or null.
  let mentionQuery = $state<{ at: number; query: string } | null>(null)
  const mentionOpen = $derived(mentionQuery !== null && mentionCandidates.length > 0)

  let mentionDebounceTimer: ReturnType<typeof setTimeout> | null = null

  /** Debounced wrapper — collapses rapid oninput/onclick calls to one execution. */
  function scheduleMentionRefresh(): void {
    if (mentionDebounceTimer) clearTimeout(mentionDebounceTimer)
    mentionDebounceTimer = setTimeout(() => {
      mentionDebounceTimer = null
      void refreshMentions()
    }, 80)
  }

  /** Recompute the `@` query + candidate list from the caret position. */
  async function refreshMentions(): Promise<void> {
    const caret = inputEl?.selectionStart ?? composerText.length
    const q = parseMentionQuery(composerText, caret)
    mentionQuery = q
    if (!q) {
      mentionCandidates = []
      return
    }
    mentionActiveIndex = 0
    // `@web <url>` keeps the menu showing just the web row until confirmed.
    if (/^web\s/i.test(q.query)) {
      mentionCandidates = SPECIAL_PROVIDERS.filter((p) => p.type === 'web')
      return
    }
    const term = q.query.toLowerCase()
    const specials = SPECIAL_PROVIDERS.filter((p) => p.insert.trim().startsWith(term))
    // Fuzzy file/folder results (line-range suffix is ignored for matching).
    const { path: matchPath } = parseLineRange(q.query)
    const index = await getWorkspaceIndex()
    const files = matchPath ? fuzzySearch(matchPath, index, 20) : []
    mentionCandidates = [...specials, ...files]
  }

  /** Splice text so the `@query` token is replaced by `replacement`. */
  function spliceMention(at: number, queryLen: number, replacement: string): void {
    const text = composerText
    const before = text.slice(0, at)
    const after = text.slice(at + 1 + queryLen) // +1 for the `@`
    const next = before + replacement + after
    setComposerText(next)
    // Restore the caret just after the inserted text.
    const pos = (before + replacement).length
    void tick().then(() => {
      if (inputEl) {
        inputEl.selectionStart = inputEl.selectionEnd = pos
        inputEl.focus()
      }
    })
  }

  /** Add a resolved mention pill (dedup by identity), clearing the query. */
  function addMention(m: MentionItem): void {
    mentions = [...mentions, m]
    mentionQuery = null
    mentionCandidates = []
  }

  /** Confirm a chosen candidate: special/file/folder → pill; @web → stays open. */
  function selectMention(c: MentionCandidate): void {
    if (!mentionQuery) return
    const { at, query } = mentionQuery
    if (c.type === 'web') {
      // If a URL was already typed (`@web https://…`), confirm it; else prime it.
      const m = /^web\s+(\S+)/i.exec(query)
      if (m) {
        spliceMention(at, query.length, '')
        addMention({
          id: crypto.randomUUID(),
          type: 'web',
          url: m[1],
          label: shortUrl(m[1]),
        })
      } else {
        spliceMention(at, query.length, '@web ')
        void tick().then(refreshMentions)
      }
      return
    }
    // Remove the typed token from the text; the pill carries the context.
    spliceMention(at, query.length, '')
    if (c.type === 'git' || c.type === 'diagnostics' || c.type === 'terminal') {
      addMention({ id: crypto.randomUUID(), type: c.type, label: c.label })
      return
    }
    // File or folder: parse a `:start-end` range off the chosen path.
    const { path: rel, lStart, lEnd } = parseLineRange(c.insert.replace(/\/$/, ''))
    const label = (rel.split('/').pop() ?? rel) + (lStart !== undefined ? `:${lStart}-${lEnd}` : '')
    addMention({
      id: crypto.randomUUID(),
      type: c.type,
      path: c.path,
      lStart,
      lEnd,
      label,
    })
  }

  function removeMention(id: string): void {
    mentions = mentions.filter((m) => m.id !== id)
  }

  /** Trim a URL to host + short path for the pill label. */
  function shortUrl(url: string): string {
    try {
      const u = new URL(url)
      return u.host + (u.pathname !== '/' ? u.pathname : '')
    } catch {
      return url
    }
  }

  /** Keydown handling for the mention menu; returns true if it consumed the key. */
  function handleMentionKeydown(e: KeyboardEvent): boolean {
    if (!mentionOpen) return false
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        mentionActiveIndex = (mentionActiveIndex + 1) % mentionCandidates.length
        return true
      case 'ArrowUp':
        e.preventDefault()
        mentionActiveIndex =
          (mentionActiveIndex - 1 + mentionCandidates.length) % mentionCandidates.length
        return true
      case 'Enter':
        e.preventDefault()
        selectMention(mentionCandidates[mentionActiveIndex] ?? mentionCandidates[0])
        return true
      case 'Tab': {
        e.preventDefault()
        selectMention(mentionCandidates[mentionActiveIndex] ?? mentionCandidates[0])
        return true
      }
      case 'Escape':
        e.preventDefault()
        mentionQuery = null
        mentionCandidates = []
        return true
      default:
        return false
    }
  }

  /** Close every inline composer picker (model/mode/persona/system). */
  function closeInlinePickers(): void {
    modelPickerOpen = false
    modePickerOpen = false
    personaPickerOpen = false
    systemEditorOpen = false
  }

  /** Execute a chosen command, routing picker commands to their inline UIs. */
  async function execSlashCommand(command: SlashCommand): Promise<void> {
    slashMenuDismissed = true
    const rest = slashQuery?.rest ?? ''
    const result = await runSlashCommand(command.id, rest)
    if (result.setInput !== undefined) setComposerText(result.setInput)
    if (
      result.openModelPicker ||
      result.openModePicker ||
      result.openPersonaPicker ||
      result.openSystemEditor
    ) {
      closeInlinePickers()
      modelPickerOpen = !!result.openModelPicker
      modePickerOpen = !!result.openModePicker
      personaPickerOpen = !!result.openPersonaPicker
      systemEditorOpen = !!result.openSystemEditor
    }
    inputEl?.focus()
  }

  /** Keydown handling for the slash menu; returns true if it consumed the key. */
  function handleSlashKeydown(e: KeyboardEvent): boolean {
    if (!slashOpen) return false
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        slashActiveIndex = (slashActiveIndex + 1) % slashCommands.length
        return true
      case 'ArrowUp':
        e.preventDefault()
        slashActiveIndex = (slashActiveIndex - 1 + slashCommands.length) % slashCommands.length
        return true
      case 'Enter':
        e.preventDefault()
        void execSlashCommand(slashCommands[slashActiveIndex] ?? slashCommands[0])
        return true
      case 'Tab': {
        // Tab completes the highlighted command name into the input.
        e.preventDefault()
        const c = slashCommands[slashActiveIndex] ?? slashCommands[0]
        if (c) setComposerText(`${c.name} `)
        return true
      }
      case 'Escape':
        e.preventDefault()
        slashMenuDismissed = true
        return true
      default:
        return false
    }
  }

  /**
   * If the composer holds a fully-typed slash command (e.g. `/clear` or
   * `/get-history 10`), run it instead of sending a message. Returns true when
   * it handled the submit.
   */
  function tryRunTypedCommand(): boolean {
    const q = parseSlashQuery(composerText)
    if (!q) return false
    const command = exactCommand(q.token)
    if (!command) return false
    void execSlashCommand(command)
    return true
  }

  function pickModel(id: string): void {
    void setModel(id)
    modelPickerOpen = false
    setComposerText('')
    inputEl?.focus()
  }
  function pickTier(tier: AgentTier): void {
    void changeAgentTier(tier)
    modePickerOpen = false
    setComposerText('')
    inputEl?.focus()
  }

  let listEl = $state<HTMLDivElement | null>(null)
  let inputEl = $state<HTMLTextAreaElement | null>(null)
  let surfaceEl = $state<HTMLDivElement | null>(null)
  let imageInputEl = $state<HTMLInputElement | null>(null)

  // Close the inline /model + /mode pickers and the @mention menu on an outside
  // click.
  $effect(() => {
    if (!modelPickerOpen && !modePickerOpen && !mentionOpen) return
    function onPointerDown(e: PointerEvent) {
      if (surfaceEl && !surfaceEl.contains(e.target as Node)) {
        modelPickerOpen = false
        modePickerOpen = false
        mentionQuery = null
        mentionCandidates = []
      }
    }
    window.addEventListener('pointerdown', onPointerDown, true)
    return () => window.removeEventListener('pointerdown', onPointerDown, true)
  })
  let dragOver = $state(false)
  // GWEN-324: conversation history is a slide-out toggled from the header.
  let historyOpen = $state(false)
  const activeConv = $derived(
    $aiChat.conversations.find((c) => c.id === $aiChat.activeConversationId) ?? null
  )

  /** Cap per image so a huge paste/drop can't bloat the request (~8 MB raw). */
  const MAX_IMAGE_BYTES = 8 * 1024 * 1024

  // Pending attachments for the next message (Requirement 14.9).
  let attachments = $state<ContextAttachment[]>([])

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

  // GWEN-334: load the per-workspace persona/system prompt whenever the open
  // folder changes (and on first mount). Falls back to defaults when there's no
  // project or no GwenLand.md. Tracks `folderPath` so it re-runs on open/close.
  $effect(() => {
    const root = $workspace.folderPath
    if (root) void loadPersona()
    else resetPersona()
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
    scheduleMentionRefresh()
  }
  function onSend() {
    // A fully-typed slash command runs instead of sending a chat message.
    if (tryRunTypedCommand()) return
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
    // Resolve any @mentions into a <context> block prepended to the message.
    const pending = mentions
    const pendingImages = images
    mentions = []
    attachments = []
    images = []
    setUnsentInput('') // empty the composer immediately while mentions resolve
    void (async () => {
      const withContext = await resolveAllMentions(text, pending)
      await sendMessage(withContext, atts, pendingImages)
    })()
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
    // A fully-typed slash command runs instead of starting an agent run.
    if (tryRunTypedCommand()) return
    if (!canSendAgent) return
    const goal = agentGoal
    const pending = mentions
    mentions = []
    agentGoal = ''
    void (async () => {
      const withContext = await resolveAllMentions(goal, pending)
      await startAgentRun(withContext)
    })()
  }
  function onKeydown(e: KeyboardEvent) {
    // Escape closes the inline /model or /mode picker first.
    if (e.key === 'Escape' && (modelPickerOpen || modePickerOpen)) {
      e.preventDefault()
      modelPickerOpen = false
      modePickerOpen = false
      return
    }
    // The @mention and slash menus claim navigation/commit keys while open.
    if (handleMentionKeydown(e)) return
    if (handleSlashKeydown(e)) return
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (isAgent) onSendAgent()
      else onSend()
    }
  }

  function attachFile() {
    const a = currentFileAttachment()
    if (a) attachments = [...attachments, a]
  }
  function attachSelection() {
    const a = currentSelectionAttachment()
    if (a) attachments = [...attachments, a]
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
    <!-- GWEN-324: sparks glyph only (no redundant "AI" text label). -->
    <span class="ai-title" title={$persona.persona.name}>
      <Icon name="sparks" size={15} />
      <span class="ai-name">{$persona.persona.name}</span>
    </span>
    <div class="ai-header-actions">
      <button
        class="icon-btn"
        title="New conversation"
        aria-label="New conversation"
        disabled={!hasProject || streaming || agentStreaming}
        onclick={createConversation}
      >
        <Icon name="plus" size={16} />
      </button>
      <button
        class="icon-btn"
        title="History"
        aria-label="Conversation history"
        onclick={() => (historyOpen = true)}
      >
        <Icon name="clock-rotate-right" size={16} />
      </button>
      <button class="icon-btn" title="Settings" aria-label="Open settings" onclick={openSettings}>
        <Icon name="settings" size={16} />
      </button>
      <button class="icon-btn" title="Close" aria-label="Close AI panel" onclick={toggleAiChat}>
        <Icon name="xmark" size={16} />
      </button>
    </div>
  </header>

  {#if activeConv}
    <div class="conv-bar">
      <span class="conv-name" title={activeConv.title}>{activeConv.title}</span>
      <Checkbox
        checked={activeConv.training_opt_in}
        onCheck={(c) => void setTrainingOptIn(activeConv.id, c)}
        title="Allow this conversation to be used as training data"
      >
        Training
      </Checkbox>
    </div>
  {/if}

  <DiffReviewPanel />

  <div class="ai-messages" bind:this={listEl}>
    {#if $aiChat.messages.length === 0}
      <div class="ai-empty-state">
        <Icon name="chat-teardrop" size={48} class="ai-empty-icon" />
        <p class="ai-empty-text">Ask anything to get started.</p>
      </div>
    {/if}
    {#each $aiChat.messages as message (message.id)}
      {#if message.agent}
        <AgentMessage {message} />
      {:else}
        <AiMessage {message} />
      {/if}
    {/each}
  </div>

  <AiHistory open={historyOpen} onClose={() => (historyOpen = false)} />

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
      bind:this={surfaceEl}
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
      <!-- Slash-command autocomplete (GWEN-333) — floats above the composer. -->
      {#if slashOpen}
        <SlashCommandMenu
          commands={slashCommands}
          activeIndex={slashActiveIndex}
          onSelect={execSlashCommand}
          onHover={(i) => (slashActiveIndex = i)}
        />
      {/if}

      <!-- @-mention autocomplete (GWEN-332) — same anchor as the slash menu. -->
      {#if mentionOpen}
        <MentionMenu
          candidates={mentionCandidates}
          activeIndex={mentionActiveIndex}
          onSelect={selectMention}
          onHover={(i) => (mentionActiveIndex = i)}
        />
      {/if}

      <!-- Inline /model picker -->
      {#if modelPickerOpen}
        <div class="slash-picker" role="listbox" aria-label="Pick model">
          <div class="slash-picker-title">Model</div>
          {#if $aiChat.models && $aiChat.models.length > 0}
            {#each $aiChat.models as m (m.id)}
              <button
                type="button"
                role="option"
                aria-selected={m.id === $aiChat.activeModel}
                class="slash-picker-row"
                class:active={m.id === $aiChat.activeModel}
                onclick={() => pickModel(m.id)}
              >
                {m.display_name || m.id}
              </button>
            {/each}
          {:else}
            <div class="slash-picker-empty">No model list for {$aiChat.activeProvider}.</div>
          {/if}
        </div>
      {/if}

      <!-- Inline /mode (autonomy tier) switcher -->
      {#if modePickerOpen}
        <div class="slash-picker" role="listbox" aria-label="Pick mode">
          <div class="slash-picker-title">Mode</div>
          {#each MODE_TIERS as t (t.id)}
            <button
              type="button"
              role="option"
              aria-selected={t.id === $agentic.tier}
              class="slash-picker-row"
              class:active={t.id === $agentic.tier}
              onclick={() => pickTier(t.id)}
            >
              <span class="slash-picker-label">{t.label}</span>
              <span class="slash-picker-hint">{t.hint}</span>
            </button>
          {/each}
        </div>
      {/if}

      <!-- Inline /persona picker (GWEN-334) -->
      {#if personaPickerOpen}
        <PersonaPicker onClose={() => (personaPickerOpen = false)} />
      {/if}

      <!-- Inline /system prompt editor (GWEN-334) -->
      {#if systemEditorOpen}
        <SystemPromptEditor onClose={() => (systemEditorOpen = false)} />
      {/if}

      <input
        type="file"
        accept="image/*"
        multiple
        bind:this={imageInputEl}
        onchange={onPickImages}
        style="display:none"
      />

      <!-- Resolved @mention pills (GWEN-332) — both chat-like and agent modes. -->
      {#if mentions.length > 0}
        <div class="mention-pills">
          {#each mentions as m (m.id)}
            <MentionPill mention={m} onRemove={() => removeMention(m.id)} />
          {/each}
        </div>
      {/if}

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
          placeholder="Ask anything..."
          value={agentGoal}
          oninput={(e) => {
            agentGoal = (e.currentTarget as HTMLTextAreaElement).value
            scheduleMentionRefresh()
          }}
          onkeydown={onKeydown}
          onclick={scheduleMentionRefresh}
          disabled={agentStreaming}
        ></textarea>
      {:else}
        <textarea
          class="ai-input"
          rows="3"
          bind:this={inputEl}
          placeholder="Ask anything..."
          value={$aiChat.unsentInput}
          oninput={onInput}
          onkeydown={onKeydown}
          onclick={scheduleMentionRefresh}
        ></textarea>
      {/if}

      <div class="ai-composer-actions">
        <ComposerActionsMenu
          {isChatLike}
          {isAgent}
          {agentStreaming}
          attachDisabled={!hasProject || streaming}
          {surfaceEl}
          onAttachFile={attachFile}
          onAttachSelection={attachSelection}
          onUploadImage={openImagePicker}
        />

        <div class="composer-controls">
          <ComposerModelMenu placement="up" />
          <ReasoningMenu placement="up" />
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
    /* Positioning context for the history slide-out (GWEN-324). */
    position: relative;
  }
  /* Active-conversation strip: title + per-conversation training toggle. */
  .conv-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 2px 16px 8px;
    flex-shrink: 0;
  }
  .conv-name {
    font-size: 12px;
    color: var(--ai-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
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
    min-width: 0;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.03em;
    color: var(--ai-text-primary);
  }
  /* Persona name in the header (GWEN-334) — truncates rather than pushing the
     header actions off-screen. */
  .ai-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .ai-empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 12px;
    color: var(--ai-text-muted);
    opacity: 0.7;
  }
  .ai-empty-text {
    font-size: 13px;
    margin: 0;
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
    position: relative;
    background-color: var(--ai-bg-surface);
    border: none;
    border-radius: 16px;
    padding: 8px 8px 6px;
    box-shadow: var(--shadow-sm);
    transition: background-color 0.12s ease;
  }
  /* Inline /model + /mode pickers, styled to match the slash menu. */
  .slash-picker {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(100% + 6px);
    z-index: 40;
    max-height: 240px;
    overflow-y: auto;
    padding: 4px;
    background-color: var(--ai-bg-surface);
    border: 1px solid var(--ai-border-subtle, transparent);
    border-radius: 12px;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.45));
    scrollbar-width: thin;
  }
  .slash-picker-title {
    padding: 4px 8px 3px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--ai-text-muted);
  }
  .slash-picker-row {
    display: flex;
    flex-direction: column;
    gap: 1px;
    width: 100%;
    padding: 5px 8px;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 7px;
    cursor: pointer;
    color: var(--ai-text-primary);
    font-size: 12px;
  }
  .slash-picker-row.active {
    color: var(--ai-primary-light);
    background-color: var(--ai-bg-hover);
  }
  .slash-picker-row:hover {
    background-color: var(--ai-bg-hover);
  }
  .slash-picker-label {
    font-weight: 600;
  }
  .slash-picker-hint {
    font-size: 11px;
    color: var(--ai-text-muted);
  }
  .slash-picker-empty {
    padding: 6px 8px;
    font-size: 11.5px;
    color: var(--ai-text-muted);
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
    container-type: inline-size;
  }
  @container (max-width: 280px) {
    .composer-controls :global(.cm-label) {
      display: none;
    }
  }
  .attach-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }
  /* Resolved @mention pills row (GWEN-332). */
  .mention-pills {
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
