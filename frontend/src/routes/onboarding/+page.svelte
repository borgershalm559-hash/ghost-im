<script lang="ts">
  import { goto } from '$app/navigation';
  import { createIdentity, openClient } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  let displayName = $state('');
  let passphrase = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    busy = true;
    errorMsg = null;
    try {
      await createIdentity(
        displayName.trim() === '' ? null : displayName.trim(),
        passphrase === '' ? null : passphrase
      );
      const info = await openClient(passphrase === '' ? null : passphrase);
      store.setInfo(info);
      await goto('/');
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section style="padding: 2rem; max-width: 520px;">
  <h1 style="margin-top: 0;">Добро пожаловать в Ghost</h1>
  <p style="opacity: 0.8;">
    Создайте свою личность. Она генерируется локально и никогда не отправляется
    на сервер. Сохраните резервную копию — без неё восстановить личность не получится.
  </p>

  <form onsubmit={submit}>
    <label style="display: block; margin: 1rem 0;">
      <div style="margin-bottom: 0.25rem;">Имя (необязательно)</div>
      <input
        type="text"
        bind:value={displayName}
        disabled={busy}
        maxlength="64"
        style="width: 100%; padding: 0.6rem; background: #1a1c22; color: inherit; border: 1px solid #2a2d36; border-radius: 6px;"
      />
    </label>

    <label style="display: block; margin: 1rem 0;">
      <div style="margin-bottom: 0.25rem;">Пароль (необязательно, рекомендуется)</div>
      <input
        type="password"
        bind:value={passphrase}
        disabled={busy}
        style="width: 100%; padding: 0.6rem; background: #1a1c22; color: inherit; border: 1px solid #2a2d36; border-radius: 6px;"
      />
    </label>

    <button
      type="submit"
      disabled={busy}
      style="padding: 0.6rem 1.2rem; background: #4a4cff; color: white; border: 0; border-radius: 6px; cursor: pointer;"
    >
      {busy ? 'Создание…' : 'Создать личность'}
    </button>
  </form>

  {#if errorMsg}
    <p style="color: #ff6464; margin-top: 1rem;">{errorMsg}</p>
  {/if}
</section>
