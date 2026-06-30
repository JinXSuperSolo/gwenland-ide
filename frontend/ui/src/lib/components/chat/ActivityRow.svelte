<script lang="ts">
  import type { AgentActivity, ActivityKind } from '../../agent-activity'
  import Icon, { type IconName } from '../Icon.svelte'

  // One row of the agent activity timeline. Dense, IDE-native, single line with
  // ellipsis so it never overflows a narrow panel.
  let { activity }: { activity: AgentActivity } = $props()

  const ICONS: Record<ActivityKind, IconName> = {
    thinking: 'brain',
    read_file: 'page',
    search: 'search',
    edit_file: 'edit-pencil',
    write_file: 'page-plus',
    run_command: 'terminal',
    approval: 'clipboard-check',
    done: 'check',
    error: 'warning-triangle',
  }
</script>

<div class="row" class:failed={activity.status === 'failed'} class:pending={activity.status === 'pending'}>
  <span class="ic"><Icon name={ICONS[activity.kind]} size={12} /></span>
  <span class="label">{activity.label}</span>
  {#if activity.detail}<span class="detail">{activity.detail}</span>{/if}
  <span class="status" aria-hidden="true">
    {#if activity.status === 'running'}<span class="spin">⟳</span>
    {:else if activity.status === 'ok'}✓
    {:else if activity.status === 'failed'}✗
    {:else}·{/if}
  </span>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 1px 0;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.5;
    color: var(--ai-text-muted);
    opacity: 0.78;
  }
  .row.failed {
    color: var(--destructive);
    opacity: 0.95;
  }
  .row.pending {
    color: var(--ai-primary-light);
    opacity: 0.95;
  }
  .ic {
    display: inline-flex;
    flex-shrink: 0;
    opacity: 0.8;
  }
  .label {
    flex-shrink: 0;
    max-width: 60%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ai-text-primary);
    opacity: 0.9;
  }
  .detail {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .status {
    flex-shrink: 0;
    margin-left: auto;
    padding-left: 6px;
  }
  .spin {
    display: inline-block;
    animation: row-spin 1.1s linear infinite;
  }
  @keyframes row-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }
</style>
