<script lang="ts" module>
  // File-type icons from material-icon-theme (full-color SVGs), imported as raw
  // strings via Vite ?raw. Curated subset for common types; everything else
  // falls back to the generic `file` icon. Folders use folder / folder-open.
  import file from 'material-icon-theme/icons/file.svg?raw'
  import folder from 'material-icon-theme/icons/folder.svg?raw'
  import folderOpen from 'material-icon-theme/icons/folder-open.svg?raw'
  import rust from 'material-icon-theme/icons/rust.svg?raw'
  import toml from 'material-icon-theme/icons/toml.svg?raw'
  import json from 'material-icon-theme/icons/json.svg?raw'
  import markdown from 'material-icon-theme/icons/markdown.svg?raw'
  import html from 'material-icon-theme/icons/html.svg?raw'
  import css from 'material-icon-theme/icons/css.svg?raw'
  import javascript from 'material-icon-theme/icons/javascript.svg?raw'
  import typescript from 'material-icon-theme/icons/typescript.svg?raw'
  import svelte from 'material-icon-theme/icons/svelte.svg?raw'
  import lock from 'material-icon-theme/icons/lock.svg?raw'
  import git from 'material-icon-theme/icons/git.svg?raw'
  import image from 'material-icon-theme/icons/image.svg?raw'
  import document from 'material-icon-theme/icons/document.svg?raw'

  // Exact filename matches (highest priority).
  const BY_NAME: Record<string, string> = {
    'cargo.toml': toml,
    'cargo.lock': lock,
    '.gitignore': git,
    'package.json': json,
    'pnpm-lock.yaml': lock,
    'tsconfig.json': json,
  }

  // Extension → icon.
  const BY_EXT: Record<string, string> = {
    rs: rust,
    toml,
    json,
    md: markdown,
    markdown,
    html: html,
    htm: html,
    css: css,
    js: javascript,
    mjs: javascript,
    cjs: javascript,
    ts: typescript,
    mts: typescript,
    svelte,
    lock,
    png: image,
    jpg: image,
    jpeg: image,
    gif: image,
    svg: image,
    webp: image,
    txt: document,
  }

  export function fileIconSvg(name: string): string {
    const lower = name.toLowerCase()
    if (BY_NAME[lower]) return BY_NAME[lower]
    const ext = lower.includes('.') ? lower.slice(lower.lastIndexOf('.') + 1) : ''
    return BY_EXT[ext] ?? file
  }

  export function folderIconSvg(open: boolean): string {
    return open ? folderOpen : folder
  }
</script>

<script lang="ts">
  let {
    name,
    dir = false,
    open = false,
    size = 16,
  }: {
    /** Filename (for files); ignored when dir=true. */
    name?: string
    dir?: boolean
    open?: boolean
    size?: number
  } = $props()

  const svg = $derived(
    (dir ? folderIconSvg(open) : fileIconSvg(name ?? ''))
      .replace(/<svg /, `<svg width="${size}" height="${size}" `),
  )
</script>

<!-- eslint-disable-next-line svelte/no-at-html-tags — trusted local SVG assets -->
<span class="file-icon" style:width={`${size}px`} style:height={`${size}px`} aria-hidden="true">
  {@html svg}
</span>

<style>
  .file-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    line-height: 0;
  }
  .file-icon :global(svg) {
    display: block;
  }
</style>
