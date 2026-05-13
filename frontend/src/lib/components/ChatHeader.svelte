<script lang="ts">
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import ContactMenu from './ContactMenu.svelte';
  import type { ContactDto } from '$lib/types';

  type Props = {
    contact: ContactDto;
  };
  let { contact }: Props = $props();

  let menu = $state<{ x: number; y: number } | null>(null);

  let displayName = $derived(
    contact.local_alias ?? contact.display_name ?? contact.fingerprint
  );

  function formatSeconds(s: number): string {
    if (s >= 86400) return `${Math.round(s / 86400)}d`;
    if (s >= 3600) return `${Math.round(s / 3600)}h`;
    if (s >= 60) return `${Math.round(s / 60)}m`;
    return `${s}s`;
  }
</script>

<header class="hdr">
  <Avatar name={displayName} size={40} />
  <div class="meta">
    <div class="name">
      <span>{displayName}</span>
      {#if contact.verified}
        <Icon name="shield" size={13} sw={2.2} color="var(--accent)" />
      {/if}
      <span class="e2e-pill">E2E</span>
    </div>
    <div class="sub">
      {contact.fingerprint}
      {#if contact.retention_seconds}
        · авто-удаление {formatSeconds(contact.retention_seconds)}
      {/if}
    </div>
  </div>
  <button
    type="button"
    class="more"
    aria-label="Действия"
    onclick={(e) => (menu = { x: e.clientX, y: e.clientY })}
  >
    <Icon name="more" size={18} color="var(--text-dim)" />
  </button>
</header>

{#if menu}
  <ContactMenu {contact} x={menu.x} y={menu.y} onClose={() => (menu = null)} />
{/if}

<style>
  .hdr {
    height: 64px;
    padding: 0 20px;
    border-bottom: 0.5px solid var(--border);
    display: flex;
    align-items: center;
    gap: 12px;
    background: var(--bg);
    flex-shrink: 0;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    letter-spacing: -0.1px;
  }
  .e2e-pill {
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--accent-dim);
    color: var(--accent);
    letter-spacing: 0.4px;
  }
  .sub {
    font-size: 12px;
    color: var(--text-dim);
    margin-top: 2px;
    font-family: 'JetBrains Mono', monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .more {
    width: 36px;
    height: 36px;
    border-radius: 9px;
    border: 0;
    cursor: pointer;
    background: transparent;
    color: var(--text-dim);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .more:hover {
    background: var(--hover);
  }
</style>
