<script lang="ts">
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import type { ContactDto } from '$lib/types';

  type Props = {
    contact: ContactDto;
    selected: boolean;
    onClick: () => void;
    onContextMenu: (x: number, y: number) => void;
  };
  let { contact, selected, onClick, onContextMenu }: Props = $props();

  let displayName = $derived(
    contact.local_alias ?? contact.display_name ?? contact.fingerprint
  );
  let timeText = $derived(formatTime(contact.last_message_at));

  function formatTime(ts: number | null): string {
    if (ts == null) return '';
    const d = new Date(ts * 1000);
    const now = new Date();
    if (d.toDateString() === now.toDateString()) {
      return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }
    const yesterday = new Date(now);
    yesterday.setDate(now.getDate() - 1);
    if (d.toDateString() === yesterday.toDateString()) return 'Вчера';
    const diff = (now.getTime() - d.getTime()) / 86_400_000;
    if (diff < 7) {
      return d.toLocaleDateString([], { weekday: 'short' });
    }
    return d.toLocaleDateString([], { day: '2-digit', month: '2-digit' });
  }
</script>

<button
  type="button"
  class="row"
  class:selected
  onclick={onClick}
  oncontextmenu={(e) => {
    e.preventDefault();
    onContextMenu(e.clientX, e.clientY);
  }}
>
  {#if selected}
    <span class="bar"></span>
  {/if}
  <Avatar name={displayName} size={36} />
  <div class="body">
    <div class="line1">
      <span class="lock"><Icon name="lock" size={11} sw={2} color="var(--success)" /></span>
      <span class="name">{displayName}</span>
      {#if contact.verified}
        <span class="badge"><Icon name="shield" size={12} sw={2} color="var(--accent)" /></span>
      {/if}
      {#if contact.muted}
        <span class="badge"><Icon name="bellOff" size={12} color="var(--text-muted)" /></span>
      {/if}
      {#if contact.pinned}
        <span class="badge"><Icon name="pin" size={11} sw={2} color="var(--text-muted)" /></span>
      {/if}
      <span class="time">{timeText}</span>
    </div>
    <div class="line2">
      <span class="last">
        {#if contact.last_message_direction === 'out'}<span class="me">Вы:&nbsp;</span>{/if}
        {contact.last_message ?? 'Нет сообщений'}
      </span>
      {#if contact.unread_count > 0}
        <span class="unread" class:muted={contact.muted}>
          {contact.unread_count > 99 ? '99+' : contact.unread_count}
        </span>
      {/if}
    </div>
  </div>
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: 10px;
    margin: 0 8px;
    background: transparent;
    border: 0;
    cursor: pointer;
    text-align: left;
    color: var(--text);
    font: inherit;
    width: calc(100% - 16px);
    position: relative;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--selected);
  }
  .bar {
    position: absolute;
    left: 0;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .line1 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 2px;
  }
  .name {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.1px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lock,
  .badge {
    display: flex;
  }
  .time {
    font-size: 12px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .line2 {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .last {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .me {
    color: var(--text-muted);
  }
  .unread {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 10px;
    background: var(--accent);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .unread.muted {
    background: var(--text-muted);
    color: var(--bg);
  }
</style>
