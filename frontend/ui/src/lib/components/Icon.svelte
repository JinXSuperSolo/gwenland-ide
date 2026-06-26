<script lang="ts" module>
  // Iconoir SVGs imported as raw strings (Vite ?raw). They use
  // stroke="currentColor", so they inherit `color` and scale via width/height.
  // Add new icons here by importing another file from iconoir/icons/regular.
  import plus from 'iconoir/icons/regular/plus.svg?raw'
  import folder from 'iconoir/icons/regular/folder.svg?raw'
  import folderPlus from 'iconoir/icons/regular/folder-plus.svg?raw'
  import xmark from 'iconoir/icons/regular/xmark.svg?raw'
  import navArrowRight from 'iconoir/icons/regular/nav-arrow-right.svg?raw'
  import navArrowDown from 'iconoir/icons/regular/nav-arrow-down.svg?raw'
  import page from 'iconoir/icons/regular/page.svg?raw'
  import pagePlus from 'iconoir/icons/regular/page-plus.svg?raw'
  import terminal from 'iconoir/icons/regular/terminal.svg?raw'
  import search from 'iconoir/icons/regular/search.svg?raw'
  import settings from 'iconoir/icons/regular/settings.svg?raw'
  import sparks from 'iconoir/icons/regular/sparks.svg?raw'
  import clockRotateRight from 'iconoir/icons/regular/clock-rotate-right.svg?raw'
  import bin from 'iconoir/icons/regular/bin.svg?raw'
  import editPencil from 'iconoir/icons/regular/edit-pencil.svg?raw'
  import sendDiagonal from 'iconoir/icons/regular/send-diagonal.svg?raw'
  import arrowUp from 'iconoir/icons/regular/arrow-up.svg?raw'
  import attachment from 'iconoir/icons/regular/attachment.svg?raw'
  import copy from 'iconoir/icons/regular/copy.svg?raw'
  import clipboardCheck from 'iconoir/icons/regular/clipboard-check.svg?raw'
  import chatBubble from 'iconoir/icons/regular/chat-bubble.svg?raw'
  import globe from 'iconoir/icons/regular/globe.svg?raw'
  import refresh from 'iconoir/icons/regular/refresh.svg?raw'
  import check from 'iconoir/icons/regular/check.svg?raw'
  import brain from 'iconoir/icons/regular/brain.svg?raw'
  // M9 context-menu icons.
  import scissor from 'iconoir/icons/regular/scissor.svg?raw'
  import code from 'iconoir/icons/regular/code.svg?raw'
  import magicWand from 'iconoir/icons/regular/magic-wand.svg?raw'
  import text from 'iconoir/icons/regular/text.svg?raw'
  import eye from 'iconoir/icons/regular/eye.svg?raw'
  import openInWindow from 'iconoir/icons/regular/open-in-window.svg?raw'
  import arrowRight from 'iconoir/icons/regular/arrow-right.svg?raw'
  import xmarkCircle from 'iconoir/icons/regular/xmark-circle.svg?raw'
  import warningTriangle from 'iconoir/icons/regular/warning-triangle.svg?raw'
  import warningCircle from 'iconoir/icons/regular/warning-circle.svg?raw'
  import gitBranch from 'iconoir/icons/regular/git-branch.svg?raw'
  import gitCommit from 'iconoir/icons/regular/git-commit.svg?raw'
  import list from 'iconoir/icons/regular/list.svg?raw'
  import reply from 'iconoir/icons/regular/reply.svg?raw'
  import collapse from 'iconoir/icons/regular/collapse.svg?raw'
  import undo from 'iconoir/icons/regular/undo.svg?raw'
  import redo from 'iconoir/icons/regular/redo.svg?raw'
  import moreHoriz from 'iconoir/icons/regular/more-horiz.svg?raw'
  import minus from 'iconoir/icons/regular/minus.svg?raw'

  export const ICONS = {
    plus,
    folder,
    'folder-plus': folderPlus,
    xmark,
    'nav-arrow-right': navArrowRight,
    'nav-arrow-down': navArrowDown,
    page,
    'page-plus': pagePlus,
    terminal,
    search,
    settings,
    sparks,
    'clock-rotate-right': clockRotateRight,
    bin,
    'edit-pencil': editPencil,
    'send-diagonal': sendDiagonal,
    'arrow-up': arrowUp,
    attachment,
    copy,
    'clipboard-check': clipboardCheck,
    'chat-bubble': chatBubble,
    globe,
    refresh,
    check,
    brain,
    scissor,
    code,
    'magic-wand': magicWand,
    text,
    eye,
    'open-in-window': openInWindow,
    'arrow-right': arrowRight,
    'xmark-circle': xmarkCircle,
    'warning-triangle': warningTriangle,
    'warning-circle': warningCircle,
    'git-branch': gitBranch,
    'git-commit': gitCommit,
    list,
    reply,
    collapse,
    undo,
    redo,
    'more-horiz': moreHoriz,
    minus,
  } as const

  export type IconName = keyof typeof ICONS
</script>

<script lang="ts">
  let {
    name,
    size = 16,
    strokeWidth = 1.6,
    class: cls = '',
  }: {
    name: IconName
    size?: number
    strokeWidth?: number
    class?: string
  } = $props()

  // Force the requested size + stroke onto the inlined <svg> root.
  const svg = $derived(
    ICONS[name]
      .replace(/\swidth="[^"]*"/, ` width="${size}"`)
      .replace(/\sheight="[^"]*"/, ` height="${size}"`)
      .replace(/\sstroke-width="[^"]*"/, ` stroke-width="${strokeWidth}"`),
  )
</script>

<!-- eslint-disable-next-line svelte/no-at-html-tags — trusted local SVG assets -->
<span class={`gw-icon ${cls}`} style:width={`${size}px`} style:height={`${size}px`} aria-hidden="true">
  {@html svg}
</span>

<style>
  .gw-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    line-height: 0;
    color: inherit;
  }
  .gw-icon :global(svg) {
    display: block;
  }
</style>
