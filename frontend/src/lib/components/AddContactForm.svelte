<script lang="ts">
  import { addContact, listContacts } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  let inviteInput = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let okMsg = $state<string | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    if (inviteInput.trim() === '') return;
    busy = true;
    errorMsg = null;
    okMsg = null;
    try {
      await addContact(inviteInput.trim());
      inviteInput = '';
      okMsg = 'Контакт добавлен.';
      const cs = await listContacts();
      store.setContacts(cs);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div style="border: 1px solid #2a2d36; border-radius: 8px; padding: 1rem; margin-bottom: 1rem;">
  <h3 style="margin: 0 0 0.5rem 0;">Добавить контакт</h3>
  <form onsubmit={submit}>
    <textarea
      bind:value={inviteInput}
      disabled={busy}
      rows="3"
      placeholder="ghostinvite1q…"
      style="width: 100%; padding: 0.5rem; background: #14151a; color: inherit; border: 1px solid #2a2d36; border-radius: 6px; font-family: monospace; font-size: 0.85rem;"
    ></textarea>
    <button
      type="submit"
      disabled={busy || inviteInput.trim() === ''}
      style="margin-top: 0.5rem; padding: 0.5rem 1rem; background: #4a4cff; color: white; border: 0; border-radius: 6px; cursor: pointer;"
    >
      {busy ? 'Добавление…' : 'Добавить'}
    </button>
  </form>
  {#if errorMsg}<p style="color: #ff6464; margin: 0.5rem 0 0 0;">{errorMsg}</p>{/if}
  {#if okMsg}<p style="color: #6effb0; margin: 0.5rem 0 0 0;">{okMsg}</p>{/if}
</div>
