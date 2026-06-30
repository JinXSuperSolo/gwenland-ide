<script lang="ts">
  import { MENUS, runCommand, shortcutFor, type MenuItem } from '../commands/registry'
  import { getRecentProjects, type RecentProject } from '../tauri/commands'
  import { openFolderPath } from '../stores/workspace'

  // Which top-level menu is open (by name), or null.
  let openMenu = $state<string | null>(null)
  let recents = $state<RecentProject[]>([])
  let recentsLoaded = $state(false)

  function toggleMenu(name: string) {
    openMenu = openMenu === name ? null : name
  }

  function openOnHover(name: string) {
    // Once a menu is open, hovering siblings switches to them (VS Code-like).
    if (openMenu !== null) openMenu = name
  }

  function close() {
    openMenu = null
  }

  async function loadRecents() {
    if (recentsLoaded) return
    try {
      recents = await getRecentProjects()
    } catch {
      recents = []
    }
    recentsLoaded = true
  }

  function itemShortcut(item: MenuItem): string | undefined {
    return item.shortcut ?? (item.commandId ? shortcutFor(item.commandId) : undefined)
  }

  function runItem(item: MenuItem) {
    if (item.disabled || item.type === 'divider' || item.children || !item.commandId) return
    close()
    void runCommand(item.commandId)
  }

  function basename(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() || p
  }
</script>

<svelte:window onclick={close} />

<nav class="menu-bar" aria-label="Menu Bar">
  {#each MENUS as menu (menu.name)}
    <div class="menu-root">
      <button
        type="button"
        class="menu-title"
        class:active={openMenu === menu.name}
        aria-haspopup="true"
        aria-expanded={openMenu === menu.name}
        onclick={(e) => {
          e.stopPropagation()
          toggleMenu(menu.name)
        }}
        onmouseenter={() => openOnHover(menu.name)}
      >
        {menu.name}
      </button>

      {#if openMenu === menu.name}
        <div class="menu-dropdown gw-anim-slide-down" role="menu" tabindex="-1">
          {#each menu.items as item}
            {#if item.type === 'divider'}
              <div class="menu-divider"></div>
            {:else if item.children === 'recent'}
              <div
                class="menu-item has-sub"
                role="menuitem"
                tabindex="0"
                onmouseenter={loadRecents}
              >
                <span class="menu-item-label">Open Recent</span>
                <span class="menu-item-arrow">▸</span>
                <div class="submenu" role="menu">
                  {#if recents.length === 0}
                    <div class="menu-item disabled">
                      <span class="menu-item-label">
                        {recentsLoaded ? 'No recent folders' : 'Loading…'}
                      </span>
                    </div>
                  {:else}
                    {#each recents as r (r.path)}
                      <button
                        type="button"
                        class="menu-item"
                        role="menuitem"
                        title={r.path}
                        onclick={() => {
                          close()
                          void openFolderPath(r.path)
                        }}
                      >
                        <span class="menu-item-label">{basename(r.path)}</span>
                      </button>
                    {/each}
                  {/if}
                </div>
              </div>
            {:else}
              <button
                type="button"
                class="menu-item"
                class:disabled={item.disabled}
                role="menuitem"
                disabled={item.disabled}
                onclick={() => runItem(item)}
              >
                <span class="menu-item-label">{item.label}</span>
                {#if itemShortcut(item)}
                  <span class="menu-item-shortcut">{itemShortcut(item)}</span>
                {/if}
              </button>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/each}
</nav>

<style>
  .menu-bar {
    height: 32px;
    flex-shrink: 0;
    display: flex;
    align-items: stretch;
    gap: 2px;
    padding: 0 8px;
    background-color: var(--background);
    border-bottom: 1px solid var(--border);
  }
  .menu-root {
    position: relative;
    display: flex;
  }
  .menu-title {
    background: transparent;
    border: none;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    padding: 0 10px;
    cursor: pointer;
  }
  .menu-title:hover,
  .menu-title.active {
    color: var(--primary);
    background-color: var(--hover-bg);
  }
  .menu-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 80;
    min-width: 240px;
    background-color: var(--popover);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-lg);
    padding: 4px;
  }
  .menu-item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 6px 10px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    position: relative;
  }
  .menu-item:not(.disabled):hover {
    background-color: var(--hover-bg);
  }
  .menu-item.disabled {
    color: var(--muted-foreground);
    opacity: 0.5;
    cursor: default;
  }
  .menu-item-shortcut,
  .menu-item-arrow {
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .menu-divider {
    height: 1px;
    background-color: var(--border);
    margin: 4px 6px;
  }
  /* Submenu (Open Recent) opens to the right on hover of its parent row. */
  .has-sub .submenu {
    display: none;
    position: absolute;
    top: -4px;
    left: 100%;
    min-width: 240px;
    background-color: var(--popover);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-lg);
    padding: 4px;
  }
  .has-sub:hover .submenu {
    display: block;
    animation: gw-slide-down 0.14s ease-out;
  }
</style>
