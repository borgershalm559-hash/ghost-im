<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import {
    identityStatus,
    openClient,
    clientInfo,
    listContacts,
    onInbound,
    getSetting,
  } from '$lib/tauri';
  import { bootTheme } from '$lib/theme';
  import { store } from '$lib/state.svelte';
  import UpdateBanner from '$lib/components/UpdateBanner.svelte';
  import ShellLayout from '$lib/components/ShellLayout.svelte';

  let { children } = $props();

  let booting = $state(true);
  let bootError = $state<string | null>(null);
  let unlistenFn: (() => void) | null = null;

  let route = $derived(page.url.pathname);
  let isOnboarding = $derived(route === '/onboarding');

  async function boot() {
    try {
      // Theme first so it's applied before any UI shows.
      const t = await bootTheme();
      store.setTheme(t);

      const status = await identityStatus();
      if (!status.exists) {
        if (!isOnboarding) await goto('/onboarding');
        return;
      }
      const info = status.client_open ? await clientInfo() : await openClient(null);
      store.setInfo(info);

      // Ghost mode (settings need open client).
      try {
        const gm = await getSetting('ghost_mode');
        store.setGhostMode(gm === '1');
      } catch {
        // ignore
      }

      const cs = await listContacts();
      store.setContacts(cs);

      const u = await onInbound(async (ev) => {
        store.pushIncoming(ev.from_ghost_id, {
          uuid: '',
          direction: 'in',
          content: ev.content,
          sent_at: ev.received_at,
          received_at: ev.received_at,
        });
        const cs = await listContacts();
        store.setContacts(cs);
      });
      unlistenFn = () => void u();
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

<UpdateBanner />

{#if booting}
  <div class="boot">Загрузка…</div>
{:else if bootError}
  <div class="boot err">{bootError}</div>
{:else if isOnboarding}
  {@render children()}
{:else}
  <ShellLayout>
    {@render children()}
  </ShellLayout>
{/if}

<style>
  .boot {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    color: var(--text-dim);
    font-size: 14px;
  }
  .boot.err {
    color: var(--danger);
    padding: 20px;
    text-align: center;
  }
</style>
