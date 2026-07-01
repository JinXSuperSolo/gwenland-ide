<script lang="ts">
  import type { ModelProvider } from '../tauri/commands'
  import { providerIconSvg } from '../icons/provider-icons'

  /** Renders a provider brand mark (GWEN-455). Mirrors `Icon.svelte`'s conventions. */
  let { provider, size = 14, class: cls = '' }: { provider: ModelProvider; size?: number; class?: string } =
    $props()

  const svg = $derived(providerIconSvg(provider, size))
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
