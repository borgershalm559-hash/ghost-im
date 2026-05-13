<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { store } from '$lib/state.svelte';
  import SearchBar from './SearchBar.svelte';
  import ChatRow from './ChatRow.svelte';
  import ContactMenu from './ContactMenu.svelte';
  import type { ContactDto } from '$lib/types';

  let menu = $state<{ contact: ContactDto; x: number; y: number } | null>(null);

  let selectedId = $derived(decodeURIComponent(page.params.ghost_id ?? ''));

  let filtered = $derived(
    store.contacts
      .filter((c) => {
        const q = store.searchQuery.trim().toLowerCase();
        if (q === '') return true;
        const name = (c.local_alias ?? c.display_name ?? '').toLowerCase();
        return (
          name.includes(q) ||
          c.fingerprint.toLowerCase().includes(q) ||
          c.ghost_id.toLowerCase().includes(q)
        );
      })
      .slice()
      .sort((a, b) => {
        if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
        const at = a.last_message_at ?? a.added_at;
        const bt = b.last_message_at ?? b.added_at;
        return bt - at;
      })
  );

  let pinned = $derived(filtered.filter((c) => c.pinned));
  let rest = $derived(filtered.filter((c) => !c.pinned));

  let totalUnread = $derived(store.contacts.reduce((s, c) => s + c.unread_count, 0));

  function open(c: ContactDto) {
    goto(`/chat/${encodeURIComponent(c.ghost_id)}`);
  }

  function pluralizeChats(n: number): string {
    const last = n % 10;
    const tens = Math.floor((n % 100) / 10);
    if (tens === 1) return 'чатов';
    if (last === 1) return 'чат';
    if (last >= 2 && last <= 4) return 'чата';
    return 'чатов';
  }
</script>

<aside class="list chat-list">
  <header>
    <div>
      <div class="title">Все</div>
      <div class="meta">
        {store.contacts.length} {pluralizeChats(store.contacts.length)}
        {#if totalUnread > 0} · {totalUnread} непрочит.{/if}
      </div>
    </div>
  </header>

  <SearchBar
    value={store.searchQuery}
    placeholder="Поиск чатов"
    onInput={(v) => store.setSearchQuery(v)}
  />

  <div class="scroll">
    {#if pinned.length > 0}
      <div class="section-label">Закреплённые</div>
      {#each pinned as c (c.ghost_id)}
        <ChatRow
          contact={c}
          selected={c.ghost_id === selectedId}
          onClick={() => open(c)}
          onContextMenu={(x, y) => (menu = { contact: c, x, y })}
        />
      {/each}
    {/if}

    {#if rest.length > 0}
      <div class="section-label">Все чаты</div>
      {#each rest as c (c.ghost_id)}
        <ChatRow
          contact={c}
          selected={c.ghost_id === selectedId}
          onClick={() => open(c)}
          onContextMenu={(x, y) => (menu = { contact: c, x, y })}
        />
      {/each}
    {/if}

    {#if store.contacts.length === 0}
      <div class="empty">Контактов пока нет.</div>
    {/if}
  </div>
</aside>

{#if menu}
  <ContactMenu
    contact={menu.contact}
    x={menu.x}
    y={menu.y}
    onClose={() => (menu = null)}
  />
{/if}

<style>
  .list {
    width: 360px;
    background: var(--sidebar);
    border-right: 0.5px solid var(--border);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }
  header {
    padding: 14px 14px 12px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .title {
    font-size: 16px;
    font-weight: 700;
    color: var(--text);
    letter-spacing: -0.3px;
  }
  .meta {
    font-size: 11px;
    color: var(--text-dim);
    margin-top: 2px;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .section-label {
    padding: 10px 18px 4px;
    font-size: 10px;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.6px;
    text-transform: uppercase;
  }
  .empty {
    padding: 40px 24px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
