<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { listMessages, sendMessage, onInbound, markChatRead, listContacts } from '$lib/tauri';
  import { store } from '$lib/state.svelte';
  import ChatHeader from './ChatHeader.svelte';
  import Composer from './Composer.svelte';
  import EncryptionBanner from './EncryptionBanner.svelte';
  import MessageBubble from './MessageBubble.svelte';
  import type { ContactDto, MessageDto } from '$lib/types';

  type Props = {
    contactGhostId: string;
  };
  let { contactGhostId }: Props = $props();

  let contact = $derived(
    store.contacts.find((c) => c.ghost_id === contactGhostId) as ContactDto | undefined
  );
  let messages = $derived<MessageDto[]>(store.threads[contactGhostId] ?? []);

  let scrollRef: HTMLDivElement | null = $state(null);
  let errorMsg = $state<string | null>(null);
  let unlisten: (() => void) | null = null;

  async function refreshContacts() {
    const cs = await listContacts();
    store.setContacts(cs);
  }

  async function loadInitial() {
    try {
      const msgs = await listMessages(contactGhostId);
      store.setThread(contactGhostId, msgs);
      await markChatRead(contactGhostId);
      await refreshContacts();
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function send(text: string) {
    await sendMessage(contactGhostId, text);
    const msgs = await listMessages(contactGhostId);
    store.setThread(contactGhostId, msgs);
    await refreshContacts();
  }

  // Effect: when contactGhostId changes, reload.
  $effect(() => {
    void contactGhostId;
    untrack(() => {
      void loadInitial();
    });
  });

  // Effect: scroll to bottom when messages change.
  $effect(() => {
    void messages;
    if (scrollRef) {
      scrollRef.scrollTop = scrollRef.scrollHeight;
    }
  });

  onMount(() => {
    void onInbound(async (ev) => {
      if (ev.from_ghost_id === contactGhostId) {
        const msgs = await listMessages(contactGhostId);
        store.setThread(contactGhostId, msgs);
        await markChatRead(contactGhostId);
        await refreshContacts();
      } else {
        await refreshContacts();
      }
    }).then((u) => {
      unlisten = u;
    });

    return () => {
      unlisten?.();
    };
  });
</script>

{#if !contact}
  <div class="loading">Загрузка контакта…</div>
{:else}
  <ChatHeader {contact} />
  <div bind:this={scrollRef} class="scroll">
    <EncryptionBanner />
    {#each messages as m, i (m.uuid || `${i}-${m.sent_at}`)}
      <MessageBubble msg={m} senderName={contact.local_alias ?? contact.display_name ?? contact.fingerprint} />
    {/each}
    {#if messages.length === 0}
      <div class="empty">Сообщений пока нет.</div>
    {/if}
  </div>
  <Composer onSend={send} />
  {#if errorMsg}<p class="err">{errorMsg}</p>{/if}
{/if}

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0 16px;
    background: var(--bg);
  }
  .empty {
    text-align: center;
    color: var(--text-muted);
    margin-top: 80px;
    font-size: 13px;
  }
  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
  }
  .err {
    color: var(--danger);
    padding: 8px 20px;
    margin: 0;
    font-size: 12px;
  }
</style>
