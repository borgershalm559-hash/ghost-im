<script lang="ts">
  import { goto } from '$app/navigation';
  import { createIdentity, openClient } from '$lib/tauri';
  import { store } from '$lib/state.svelte';
  import Icon from '$lib/components/Icon.svelte';

  let displayName = $state('');
  let passphrase = $state('');
  let passphraseConfirm = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    errorMsg = null;
    if (passphrase !== passphraseConfirm) {
      errorMsg = 'Пароли не совпадают.';
      return;
    }
    busy = true;
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

<div class="root">
  <div class="bg"></div>
  <main class="card">
    <div class="logo">
      <Icon name="ghost" size={56} sw={1.5} color="var(--accent)" />
    </div>
    <h1>Добро пожаловать в Ghost</h1>
    <p class="lead">
      Анонимный, end-to-end зашифрованный мессенджер. Ваша личность создаётся
      локально — никакие серверы её не видят.
    </p>

    <form onsubmit={submit}>
      <label>
        <span class="lbl">Имя (необязательно)</span>
        <input
          type="text"
          bind:value={displayName}
          disabled={busy}
          maxlength="64"
          placeholder="Как вас называть"
        />
      </label>

      <label>
        <span class="lbl">Пароль (рекомендуется)</span>
        <input
          type="password"
          bind:value={passphrase}
          disabled={busy}
          placeholder="Защищает identity-файл"
        />
        <span class="hint">
          Без пароля identity защищается только OS-keystore'ом. С паролем — двумя
          слоями.
        </span>
      </label>

      {#if passphrase !== ''}
        <label>
          <span class="lbl">Повторите пароль</span>
          <input
            type="password"
            bind:value={passphraseConfirm}
            disabled={busy}
            placeholder="Ещё раз"
          />
        </label>
      {/if}

      <button type="submit" class="primary" disabled={busy}>
        {busy ? 'Создание…' : 'Создать личность'}
      </button>

      {#if errorMsg}
        <p class="err">{errorMsg}</p>
      {/if}

      <p class="hint center">
        <Icon name="lock" size={12} sw={2} color="var(--accent)" /> Локально ·
        Argon2id + XChaCha20-Poly1305 · 0 серверных round-trip'ов
      </p>
    </form>
  </main>
</div>

<style>
  .root {
    flex: 1;
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    overflow: auto;
    padding: 32px 16px;
  }
  .bg {
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at 50% 30%, var(--accent-dim) 0%, transparent 60%);
    pointer-events: none;
  }
  .card {
    background: var(--surface);
    border: 0.5px solid var(--border);
    border-radius: 16px;
    padding: 36px 36px 28px;
    max-width: 480px;
    width: 100%;
    position: relative;
    z-index: 1;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.4);
  }
  .logo {
    text-align: center;
    margin-bottom: 12px;
  }
  h1 {
    margin: 0;
    text-align: center;
    font-size: 22px;
    font-weight: 700;
    color: var(--text);
    letter-spacing: -0.4px;
  }
  .lead {
    margin: 10px 0 24px;
    text-align: center;
    color: var(--text-dim);
    line-height: 1.6;
    font-size: 13px;
  }
  label {
    display: block;
    margin-bottom: 14px;
  }
  .lbl {
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-dim);
    margin-bottom: 6px;
  }
  input {
    width: 100%;
    padding: 10px 12px;
    background: var(--bg);
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 10px;
    font-size: 14px;
    font-family: inherit;
    outline: none;
  }
  input:focus {
    border-color: var(--accent);
  }
  .hint {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 4px;
    display: block;
    line-height: 1.5;
  }
  .hint.center {
    text-align: center;
    margin-top: 16px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    justify-content: center;
  }
  .primary {
    width: 100%;
    padding: 12px;
    background: linear-gradient(135deg, #6c5ce7, var(--accent));
    color: #fff;
    border: 0;
    border-radius: 10px;
    cursor: pointer;
    font-weight: 600;
    font-size: 14px;
    margin-top: 6px;
    box-shadow: 0 8px 24px var(--accent-soft);
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    margin: 10px 0 0 0;
    text-align: center;
  }
</style>
