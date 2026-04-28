<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { identityStatus, openClient, clientInfo } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  let booting = $state(true);
  let bootError = $state<string | null>(null);

  onMount(async () => {
    try {
      const status = await identityStatus();
      if (!status.exists) {
        await goto('/onboarding');
        return;
      }
      if (!status.client_open) {
        const info = await openClient(null);
        store.setInfo(info);
      } else {
        const info = await clientInfo();
        store.setInfo(info);
      }
    } catch (e) {
      bootError = String(e);
    } finally {
      booting = false;
    }
  });
</script>

<section style="padding: 2rem;">
  {#if booting}
    <p>Loading…</p>
  {:else if bootError}
    <p style="color: #ff6464;">Failed to load: {bootError}</p>
    <p style="opacity: 0.7;">If you set a passphrase, the open-client flow needs UI for entering it. Coming in Plan 08.</p>
  {:else if store.info}
    <p>Signed in as {store.info.fingerprint}</p>
    <pre style="background: #1a1c22; padding: 1rem; border-radius: 6px; overflow: auto;">{store.info.ghost_id}</pre>
  {/if}
</section>
