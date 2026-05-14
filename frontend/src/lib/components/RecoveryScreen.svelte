<script lang="ts">
  import Icon from './Icon.svelte';
  import { openClient, importBackup, listContacts } from '$lib/tauri';
  import { store } from '$lib/state.svelte';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';

  type Props = {
    errorText: string;
    onRecovered: () => void;
  };
  let { errorText, onRecovered }: Props = $props();

  type Mode = 'choice' | 'passphrase' | 'restore' | 'wipe';
  let mode = $state<Mode>('choice');

  let passphrase = $state('');
  let busy = $state(false);
  let localError = $state<string | null>(null);

  let restorePassphrase = $state('');
  let restoreOk = $state(false);

  async function tryPassphrase() {
    if (passphrase === '' || busy) return;
    busy = true;
    localError = null;
    try {
      const info = await openClient(passphrase);
      store.setInfo(info);
      const cs = await listContacts();
      store.setContacts(cs);
      onRecovered();
    } catch (e) {
      localError = String(e);
    } finally {
      busy = false;
    }
  }

  async function tryRestore() {
    if (restorePassphrase === '' || busy) return;
    busy = true;
    localError = null;
    try {
      const sel = await openDialog({
        title: 'Выбрать файл бэкапа',
        multiple: false,
        filters: [{ name: 'Ghost backup', extensions: ['ghost-backup'] }],
      });
      if (!sel || typeof sel !== 'string') {
        busy = false;
        return;
      }
      await importBackup(sel, restorePassphrase);
      restoreOk = true;
      restorePassphrase = '';
    } catch (e) {
      localError = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="root">
  <div class="bg"></div>
  <main class="card">
    <div class="icon-wrap">
      <Icon name="lock" size={40} sw={2} color="var(--danger)" />
    </div>
    <h1>Не получается открыть identity</h1>
    <p class="lead">
      Identity-файл существует, но ключ для его расшифровки не совпадает. Это
      бывает, если установка Ghost обновилась и потеряла секрет в Windows
      Credential Manager, либо если identity создан под паролем.
    </p>

    <div class="err-block">
      <span class="err-label">Детали:</span>
      <code>{errorText}</code>
    </div>

    {#if mode === 'choice'}
      <div class="buttons">
        <button type="button" class="primary" onclick={() => (mode = 'passphrase')}>
          Войти с паролем
        </button>
        <button type="button" class="secondary" onclick={() => (mode = 'restore')}>
          Восстановить из бэкапа
        </button>
        <button type="button" class="danger" onclick={() => (mode = 'wipe')}>
          Начать заново (стереть данные)
        </button>
      </div>
    {:else if mode === 'passphrase'}
      <div class="recovery-form">
        <label>
          <span class="lbl">Введите пароль identity</span>
          <input
            type="password"
            bind:value={passphrase}
            disabled={busy}
            onkeydown={(e) => {
              if (e.key === 'Enter') void tryPassphrase();
            }}
            placeholder="Пароль вашего identity-файла"
          />
        </label>
        <div class="row">
          <button type="button" class="secondary" onclick={() => (mode = 'choice')}>
            ← Назад
          </button>
          <button type="button" class="primary" disabled={busy} onclick={tryPassphrase}>
            {busy ? 'Проверка…' : 'Открыть'}
          </button>
        </div>
        {#if localError}<p class="err">{localError}</p>{/if}
      </div>
    {:else if mode === 'restore'}
      <div class="recovery-form">
        {#if restoreOk}
          <p class="ok">
            Identity восстановлен из бэкапа. Перезапустите приложение для
            применения изменений.
          </p>
          <p class="hint">
            Закройте Ghost (через крестик) и откройте снова.
          </p>
        {:else}
          <label>
            <span class="lbl">Пароль от бэкапа</span>
            <input
              type="password"
              bind:value={restorePassphrase}
              disabled={busy}
              onkeydown={(e) => {
                if (e.key === 'Enter') void tryRestore();
              }}
              placeholder="Тот пароль, что вы задали при создании бэкапа"
            />
          </label>
          <div class="row">
            <button type="button" class="secondary" onclick={() => (mode = 'choice')}>
              ← Назад
            </button>
            <button type="button" class="primary" disabled={busy} onclick={tryRestore}>
              {busy ? 'Восстановление…' : 'Выбрать файл бэкапа…'}
            </button>
          </div>
          {#if localError}<p class="err">{localError}</p>{/if}
        {/if}
      </div>
    {:else if mode === 'wipe'}
      <div class="recovery-form">
        <p class="warn">
          ⚠️ Это удалит ваш identity и всю историю сообщений безвозвратно.
          Сделайте это только если у вас нет бэкапа.
        </p>
        <p class="hint">
          После «Стереть» закройте приложение, удалите вручную папку
          <code>%APPDATA%\Ghost</code>, перезапустите Ghost — увидите онбординг.
        </p>
        <div class="row">
          <button type="button" class="secondary" onclick={() => (mode = 'choice')}>
            ← Назад
          </button>
        </div>
      </div>
    {/if}
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
    background: radial-gradient(circle at 50% 30%, rgba(255, 107, 122, 0.10) 0%, transparent 55%);
    pointer-events: none;
  }
  .card {
    background: var(--surface);
    border: 0.5px solid var(--border);
    border-radius: 16px;
    padding: 32px;
    max-width: 540px;
    width: 100%;
    position: relative;
    z-index: 1;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.4);
  }
  .icon-wrap {
    text-align: center;
    margin-bottom: 12px;
  }
  h1 {
    margin: 0 0 12px;
    font-size: 20px;
    font-weight: 700;
    color: var(--text);
    text-align: center;
    letter-spacing: -0.3px;
  }
  .lead {
    margin: 0 0 16px;
    color: var(--text-dim);
    line-height: 1.6;
    font-size: 13px;
    text-align: center;
  }
  .err-block {
    background: var(--bg);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
    margin-bottom: 20px;
  }
  .err-label {
    display: block;
    font-size: 10px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.6px;
    margin-bottom: 4px;
  }
  .err-block code {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    color: var(--danger);
    word-break: break-word;
  }
  .buttons {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .primary,
  .secondary,
  .danger {
    padding: 12px;
    border: 0;
    border-radius: 10px;
    cursor: pointer;
    font-weight: 600;
    font-size: 13px;
    font-family: inherit;
  }
  .primary {
    background: linear-gradient(135deg, #6c5ce7, var(--accent));
    color: #fff;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .secondary {
    background: transparent;
    color: var(--text);
    border: 0.5px solid var(--border);
  }
  .secondary:hover {
    background: var(--hover);
  }
  .danger {
    background: transparent;
    color: var(--danger);
    border: 0.5px solid rgba(255, 107, 122, 0.4);
  }
  .danger:hover {
    background: rgba(255, 107, 122, 0.08);
  }
  .recovery-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  label {
    display: block;
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
  .row {
    display: flex;
    gap: 8px;
  }
  .row button {
    flex: 1;
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    margin: 0;
    padding: 8px 12px;
    background: rgba(255, 107, 122, 0.08);
    border-radius: 8px;
  }
  .ok {
    color: var(--success);
    font-size: 13px;
    margin: 0;
    padding: 12px;
    background: rgba(61, 220, 151, 0.08);
    border-radius: 10px;
  }
  .hint {
    font-size: 12px;
    color: var(--text-dim);
    margin: 0;
    line-height: 1.6;
  }
  .hint code {
    background: var(--bg);
    padding: 1px 4px;
    border-radius: 4px;
    font-size: 11px;
    font-family: 'JetBrains Mono', monospace;
  }
  .warn {
    color: var(--danger);
    font-size: 13px;
    margin: 0;
    padding: 12px;
    background: rgba(255, 107, 122, 0.06);
    border-radius: 10px;
    line-height: 1.5;
  }
</style>
