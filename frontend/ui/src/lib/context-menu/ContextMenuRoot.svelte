<script lang="ts">
  // The one mount point for the whole context-menu system. Mounted once at the
  // app root; it always renders the portal, which self-hides until a surface
  // calls `openContextMenu`. Surfaces never mount a menu of their own.
  import ContextMenuPortal from './ContextMenuPortal.svelte'
</script>

<ContextMenuPortal />
