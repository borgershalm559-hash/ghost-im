<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { listMessages, sendMessage, onInbound } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  let contactGhostId = $derived(decodeURIComponent(page.params.ghost_id ?? ''));
  let messages = $derived(store.threads[contactGhostId] ?? []);
  let input = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let unlisten: (() => void) | null = null;
  let scrollRef: HTMLDivElement | null = $state(null);

  $effect(() => {
    void messages;
    if (scrollRef) {
      scrollRef.scrollTop = scrollRef.scrollHeight;
    }
  });

  onMount(() => {
    const init = async () => {
      try {
        const initial = await listMessages(contactGhostId);
        store.setThread(contactGhostId, initial);
      } catch (e) {
        errorMsg = String(e);
      }

      const u = await onInbound((ev) => {
        if (ev.from_ghost_id === contactGhostId) {
          store.pushIncoming(contactGhostId, {
            uuid: '',
            direction: 'in',
            content: ev.content,
            sent_at: ev.received_at,
            received_at: ev.received_at
          });
        }
      });
      unlisten = u;
    };

    void init();

    return () => {
      unlisten?.();
    };
  });

  async function submit(e: Event) {
    e.preventDefault();
    const text = input.trim();
    if (text === '') return;
    busy = true;
    errorMsg = null;
    try {
      await sendMessage(contactGhostId, text);
      const refreshed = await listMessages(contactGhostId);
      store.setThread(contactGhostId, refreshed);
      input = '';
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section style="display: flex; flex-direction: column; height: 100%; padding: 1rem; max-width: 720px; margin: 0 auto;">
  <header style="margin-bottom: 0.75rem;">
    <a href="/" style="color: #4a8cff; text-decoration: none;">← Home</a>
    <div style="font-family: monospace; opacity: 0.7; font-size: 0.85rem; margin-top: 0.25rem; word-break: break-all;">
      {contactGhostId}
    </div>
  </header>

  <div
    bind:this={scrollRef}
    style="flex: 1; overflow-y: auto; padding: 0.5rem; background: #14151a; border: 1px solid #2a2d36; border-radius: 6px;"
  >
    {#each messages as m, i (m.uuid || `${i}-${m.sent_at}`)}
      <div
        style="margin: 0.4rem 0; display: flex; {m.direction === 'out' ? 'justify-content: flex-end' : 'justify-content: flex-start'};"
      >
        <div
          style="max-width: 70%; padding: 0.5rem 0.75rem; border-radius: 8px; background: {m.direction === 'out' ? '#4a4cff' : '#23252e'}; color: inherit; word-wrap: break-word;"
        >
          {m.content}
        </div>
      </div>
    {/each}
    {#if messages.length === 0}
      <p style="opacity: 0.5; text-align: center; margin-top: 2rem;">No messages yet.</p>
    {/if}
  </div>

  <form
    onsubmit={submit}
    style="display: flex; gap: 0.5rem; margin-top: 0.75rem;"
  >
    <input
      type="text"
      bind:value={input}
      disabled={busy}
      placeholder="Type a message…"
      style="flex: 1; padding: 0.6rem; background: #14151a; color: inherit; border: 1px solid #2a2d36; border-radius: 6px;"
    />
    <button
      type="submit"
      disabled={busy || input.trim() === ''}
      style="padding: 0.6rem 1.2rem; background: #4a4cff; color: white; border: 0; border-radius: 6px; cursor: pointer;"
    >
      {busy ? 'Sending…' : 'Send'}
    </button>
  </form>

  {#if errorMsg}<p style="color: #ff6464; margin: 0.5rem 0 0 0;">{errorMsg}</p>{/if}
</section>
