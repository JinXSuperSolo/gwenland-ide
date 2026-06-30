<script lang="ts" module>
  export type { IconName } from '../icons/gwenland-icons'
</script>

<script lang="ts">
  import { getIcon, type IconName } from '../icons/gwenland-icons'

  let {
    name,
    size = 16,
    strokeWidth = 1.6,
    color = 'currentColor',
    class: cls = '',
  }: {
    name: IconName
    size?: number
    strokeWidth?: number
    color?: string
    class?: string
  } = $props()

  const svg = $derived(getIcon(name, size, color, strokeWidth))
</script>

<!-- eslint-disable-next-line svelte/no-at-html-tags - trusted local SVG assets -->
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
