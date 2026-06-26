<script lang="ts" module>
  import { fileIconSvg, folderIconSvg } from '../icons/gwenland-icons'
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
    (dir ? folderIconSvg(open) : fileIconSvg(name ?? '')).replace(
      /<svg /,
      `<svg width="${size}" height="${size}" `,
    ),
  )
</script>

<!-- eslint-disable-next-line svelte/no-at-html-tags - trusted local SVG assets -->
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
