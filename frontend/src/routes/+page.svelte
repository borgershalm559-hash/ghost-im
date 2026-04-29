<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    identityStatus,
    openClient,
    clientInfo,
    listContacts,
    onInbound
  } from '$lib/tauri';
  import { store } from '$lib/state.svelte';
  import InviteCard from '$lib/components/InviteCard.svelte';
  import AddContactForm from '$lib/components/AddContactForm.svelte';
  import ContactList from '$lib/components/ContactList.svelte';

  let booting = $state(true);
  let bootError = $state<string | null>(null);
  let unlistenFn: (() => void) | null = null;

  async function boot() {
    try {
      const status = await identityStatus();
      if (!status.exists) {
        await goto('/onboarding');
        return;
      }
      const info = status.client_open ? await clientInfo() : await openClient(null);
      store.setInfo(info);

      const cs = await listContacts();
      store.setContacts(cs);

      const u = await onInbound((ev) => {
        store.pushIncoming(ev.from_ghost_id, {
          uuid: '',
          direction: 'in',
          content: ev.content,
          sent_at: ev.received_at,
          received_at: ev.received_at
        });
      });
      unlistenFn = () => {
        void u();
      };
    } catch (e) {
      bootError = String(e);
    } finally {
      booting = false;
    }
  }

  onMount(() => {
    void boot();
    return () => {
      unlistenFn?.();
    };
  });
</script>

<section style="padding: 2rem; max-width: 720px; margin: 0 auto;">
  {#if booting}
    <p>Загрузка…</p>
  {:else if bootError}
    <p style="color: #ff6464;">{bootError}</p>
  {:else if store.info}
    <header style="margin-bottom: 1.5rem;">
      <div style="opacity: 0.6; font-size: 0.8rem;">ВАШ GHOST ID</div>
      <div style="font-family: monospace; font-size: 0.95rem; word-break: break-all;">
        {store.info.ghost_id}
      </div>
      <div style="font-family: monospace; opacity: 0.7; font-size: 0.85rem; margin-top: 0.25rem;">
        {store.info.fingerprint}
      </div>
    </header>

    <InviteCard />
    <AddContactForm />
    <ContactList />
  {/if}
</section>
