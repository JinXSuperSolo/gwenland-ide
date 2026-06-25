<script lang="ts">
  import EditorGroups from './EditorGroups.svelte'
</script>

<main class="workspace" aria-label="Workspace">
  <EditorGroups />
</main>

<style>
  .workspace {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background-color: var(--background);
  }
</style>
