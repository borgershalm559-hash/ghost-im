<script lang="ts">
  import { goto } from '$app/navigation';
  import { store } from '$lib/state.svelte';

  function open(ghost_id: string) {
    goto(`/chat/${encodeURIComponent(ghost_id)}`);
  }
</script>

<div style="border: 1px solid #2a2d36; border-radius: 8px; padding: 1rem;">
  <h3 style="margin: 0 0 0.5rem 0;">Contacts</h3>
  {#if store.contacts.length === 0}
    <p style="opacity: 0.6; margin: 0;">No contacts yet. Share an invite to add one.</p>
  {:else}
    <ul style="list-style: none; padding: 0; margin: 0;">
      {#each store.contacts as c (c.ghost_id)}
        <li style="margin-bottom: 0.5rem;">
          <button
            type="button"
            onclick={() => open(c.ghost_id)}
            style="display: block; width: 100%; text-align: left; padding: 0.6rem; background: #14151a; color: inherit; border: 1px solid #2a2d36; border-radius: 6px; cursor: pointer;"
          >
            <div style="font-family: monospace; font-size: 0.85rem;">{c.fingerprint}</div>
            <div style="opacity: 0.6; font-size: 0.75rem; word-break: break-all;">{c.ghost_id}</div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
