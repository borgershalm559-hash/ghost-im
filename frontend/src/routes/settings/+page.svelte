<script lang="ts">
  import { goto } from '$app/navigation';
  import { store } from '$lib/state.svelte';
  import { persistTheme } from '$lib/theme';
  import { setSetting, exportBackup, importBackup, getDiagnostics } from '$lib/tauri';
  import type { DiagnosticsDto } from '$lib/tauri';
  import Icon from '$lib/components/Icon.svelte';
  import IdentityModal from '$lib/components/IdentityModal.svelte';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

  type Section = 'profile' | 'appearance' | 'security' | 'diagnostics' | 'about';
  let section = $state<Section>('profile');

  let diagnostics = $state<DiagnosticsDto | null>(null);
  let diagBusy = $state(false);
  let diagError = $state<string | null>(null);

  async function loadDiagnostics() {
    diagBusy = true;
    diagError = null;
    try {
      diagnostics = await getDiagnostics();
    } catch (e) {
      diagError = String(e);
    } finally {
      diagBusy = false;
    }
  }

  $effect(() => {
    if (section === 'diagnostics' && diagnostics === null && !diagBusy) {
      void loadDiagnostics();
    }
  });

  async function copyDiagnosticsToClipboard() {
    if (!diagnostics) return;
    const text = `Ghost ${diagnostics.app_version}
Ghost ID: ${diagnostics.ghost_id}
Fingerprint: ${diagnostics.fingerprint}
Peer ID: ${diagnostics.peer_id}
Local addresses:
${diagnostics.local_addrs.map((a) => '  - ' + a).join('\n')}
Bootstrap nodes: ${diagnostics.bootstrap_count}`;
    await navigator.clipboard.writeText(text);
  }

  let identityOpen = $state(false);

  // Backup state
  let backupPassphrase = $state('');
  let backupBusy = $state(false);
  let backupMsg = $state<{ ok: boolean; text: string } | null>(null);

  let restoreBusy = $state(false);
  let restorePassphrase = $state('');
  let restoreMsg = $state<{ ok: boolean; text: string } | null>(null);

  async function pickTheme(t: 'dark' | 'light') {
    await persistTheme(t);
    store.setTheme(t);
  }

  async function toggleGhostMode() {
    const next = !store.ghostMode;
    store.setGhostMode(next);
    try {
      await setSetting('ghost_mode', next ? '1' : '0');
    } catch {
      // ignore
    }
  }

  async function doExport() {
    backupMsg = null;
    if (backupPassphrase.length < 6) {
      backupMsg = { ok: false, text: 'Пароль должен быть минимум 6 символов.' };
      return;
    }
    const path = await saveDialog({
      title: 'Сохранить бэкап Ghost',
      defaultPath: `ghost-backup-${new Date().toISOString().slice(0, 10)}.ghost-backup`,
      filters: [{ name: 'Ghost backup', extensions: ['ghost-backup'] }],
    });
    if (!path) return;
    backupBusy = true;
    try {
      const size = await exportBackup(path, backupPassphrase);
      backupMsg = { ok: true, text: `Бэкап сохранён (${(size / 1024).toFixed(1)} КБ).` };
      backupPassphrase = '';
    } catch (e) {
      backupMsg = { ok: false, text: String(e) };
    } finally {
      backupBusy = false;
    }
  }

  async function doImport() {
    restoreMsg = null;
    if (!restorePassphrase) {
      restoreMsg = { ok: false, text: 'Введите пароль от бэкапа.' };
      return;
    }
    const sel = await openDialog({
      title: 'Выбрать файл бэкапа',
      multiple: false,
      filters: [{ name: 'Ghost backup', extensions: ['ghost-backup'] }],
    });
    if (!sel || typeof sel !== 'string') return;
    restoreBusy = true;
    try {
      await importBackup(sel, restorePassphrase);
      restoreMsg = {
        ok: true,
        text:
          'Восстановление успешно. Закройте и снова откройте приложение, чтобы применить.',
      };
      restorePassphrase = '';
    } catch (e) {
      restoreMsg = { ok: false, text: String(e) };
    } finally {
      restoreBusy = false;
    }
  }
</script>

<div class="page">
  <header class="hdr">
    <button class="back" type="button" onclick={() => goto('/')} aria-label="Назад">
      <span style="font-size: 18px;">←</span>
    </button>
    <h1>Настройки</h1>
  </header>

  <div class="body">
    <nav class="nav">
      <button class:active={section === 'profile'} onclick={() => (section = 'profile')}>
        Профиль
      </button>
      <button class:active={section === 'appearance'} onclick={() => (section = 'appearance')}>
        Внешний вид
      </button>
      <button class:active={section === 'security'} onclick={() => (section = 'security')}>
        Безопасность
      </button>
      <button class:active={section === 'diagnostics'} onclick={() => (section = 'diagnostics')}>
        Диагностика
      </button>
      <button class:active={section === 'about'} onclick={() => (section = 'about')}>
        О программе
      </button>
    </nav>

    <section class="content">
      {#if section === 'profile'}
        <h2>Профиль</h2>
        <div class="card">
          <div class="row">
            <div>
              <div class="row-title">Ghost ID</div>
              <div class="row-sub">
                Ваш публичный ID. Поделитесь им с контактами через инвайт.
              </div>
            </div>
            <button class="primary" onclick={() => (identityOpen = true)}>Показать</button>
          </div>
        </div>

      {:else if section === 'appearance'}
        <h2>Внешний вид</h2>
        <div class="card">
          <div class="row">
            <div>
              <div class="row-title">Тема</div>
              <div class="row-sub">Светлая или тёмная — переключается мгновенно.</div>
            </div>
            <div class="seg">
              <button
                class:active={store.theme === 'dark'}
                onclick={() => pickTheme('dark')}>Тёмная</button
              >
              <button
                class:active={store.theme === 'light'}
                onclick={() => pickTheme('light')}>Светлая</button
              >
            </div>
          </div>
          <div class="row">
            <div>
              <div class="row-title">Ghost mode</div>
              <div class="row-sub">
                Помечает вас как «invisible» (визуально). Реальная скрытность от
                сети появится в следующих версиях.
              </div>
            </div>
            <button class="toggle" class:on={store.ghostMode} onclick={toggleGhostMode}>
              <span class="knob"></span>
            </button>
          </div>
        </div>

      {:else if section === 'security'}
        <h2>Безопасность</h2>
        <div class="card">
          <div class="row col">
            <div class="row-title">Создать резервную копию</div>
            <div class="row-sub">
              Экспортирует identity + базу сообщений в зашифрованный файл
              <code>.ghost-backup</code>. Сохраните его в надёжное место —
              без него восстановить аккаунт после потери устройства невозможно.
            </div>
            <div class="input-row">
              <input
                type="password"
                placeholder="Пароль для бэкапа (мин. 6 симв.)"
                bind:value={backupPassphrase}
                disabled={backupBusy}
              />
              <button class="primary" disabled={backupBusy} onclick={doExport}>
                {backupBusy ? 'Создание…' : 'Создать бэкап'}
              </button>
            </div>
            {#if backupMsg}
              <p class:ok={backupMsg.ok} class:err={!backupMsg.ok}>{backupMsg.text}</p>
            {/if}
          </div>
        </div>

        <div class="card">
          <div class="row col">
            <div class="row-title">Восстановить из бэкапа</div>
            <div class="row-sub">
              Выберите файл <code>.ghost-backup</code> и введите пароль. После
              восстановления приложение нужно перезапустить.
            </div>
            <div class="input-row">
              <input
                type="password"
                placeholder="Пароль от бэкапа"
                bind:value={restorePassphrase}
                disabled={restoreBusy}
              />
              <button class="primary" disabled={restoreBusy} onclick={doImport}>
                {restoreBusy ? 'Восстановление…' : 'Выбрать файл…'}
              </button>
            </div>
            {#if restoreMsg}
              <p class:ok={restoreMsg.ok} class:err={!restoreMsg.ok}>{restoreMsg.text}</p>
            {/if}
          </div>
        </div>

      {:else if section === 'diagnostics'}
        <h2>Диагностика</h2>
        <p class="row-sub" style="margin: 0 0 14px 0;">
          Техническая информация о вашем подключении. Полезно для багрепортов и
          отладки сетевых проблем.
        </p>

        {#if diagBusy}
          <div class="card">
            <div class="row">Загрузка…</div>
          </div>
        {:else if diagError}
          <div class="card">
            <p class="err">{diagError}</p>
            <button class="primary" onclick={loadDiagnostics}>Повторить</button>
          </div>
        {:else if diagnostics}
          <div class="card">
            <div class="diag-row">
              <div class="diag-label">Версия приложения</div>
              <div class="diag-value">{diagnostics.app_version}</div>
            </div>
            <div class="diag-row">
              <div class="diag-label">Peer ID (libp2p)</div>
              <div class="diag-value mono">{diagnostics.peer_id}</div>
            </div>
            <div class="diag-row">
              <div class="diag-label">Bootstrap-нод подключено</div>
              <div class="diag-value">{diagnostics.bootstrap_count}</div>
            </div>
            <div class="diag-row col">
              <div class="diag-label">Локальные адреса (libp2p слушает)</div>
              <div class="diag-addrs">
                {#each diagnostics.local_addrs as addr}
                  <code>{addr}</code>
                {/each}
                {#if diagnostics.local_addrs.length === 0}
                  <span class="dim">нет</span>
                {/if}
              </div>
            </div>
          </div>
          <button class="primary" style="margin-top: 8px;" onclick={copyDiagnosticsToClipboard}>
            Скопировать в буфер
          </button>
          <button
            class="secondary"
            style="margin-top: 8px; margin-left: 8px;"
            onclick={loadDiagnostics}
          >
            Обновить
          </button>
        {/if}

      {:else if section === 'about'}
        <h2>О программе</h2>
        <div class="card">
          <div class="about-logo">
            <Icon name="ghost" size={48} sw={1.5} color="var(--accent)" />
          </div>
          <div class="about-name">Ghost</div>
          <div class="about-version">Версия 0.0.6</div>
          <div class="about-desc">
            Анонимный, end-to-end зашифрованный десктоп-мессенджер. Гибрид
            Discord и Telegram, не требует хостинга. Federated и open-source.
          </div>
          <div class="about-links">
            <a href="https://github.com/borgershalm559-hash/ghost-im" target="_blank" rel="noopener">
              GitHub →
            </a>
            <span class="dot">·</span>
            <a
              href="https://github.com/borgershalm559-hash/ghost-im/blob/master/docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md"
              target="_blank"
              rel="noopener">Дизайн MVP-1 →</a
            >
            <span class="dot">·</span>
            <a
              href="https://github.com/borgershalm559-hash/ghost-im/releases"
              target="_blank"
              rel="noopener">Релизы →</a
            >
          </div>
          <div class="about-license">AGPL-3.0-only · © 2026</div>
        </div>
      {/if}
    </section>
  </div>
</div>

<IdentityModal open={identityOpen} onClose={() => (identityOpen = false)} />

<style>
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .hdr {
    height: 56px;
    border-bottom: 0.5px solid var(--border);
    display: flex;
    align-items: center;
    padding: 0 16px;
    gap: 8px;
    background: var(--bg);
    flex-shrink: 0;
  }
  .hdr h1 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text);
  }
  .back {
    width: 36px;
    height: 36px;
    border: 0;
    background: transparent;
    color: var(--text);
    border-radius: 8px;
    cursor: pointer;
  }
  .back:hover {
    background: var(--hover);
  }
  .body {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .nav {
    width: 200px;
    padding: 16px 8px;
    border-right: 0.5px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex-shrink: 0;
  }
  .nav button {
    border: 0;
    background: transparent;
    color: var(--text-dim);
    padding: 8px 12px;
    border-radius: 8px;
    text-align: left;
    cursor: pointer;
    font-size: 13px;
  }
  .nav button:hover {
    background: var(--hover);
  }
  .nav button.active {
    background: var(--accent-dim);
    color: var(--accent);
    font-weight: 600;
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 24px 32px;
    max-width: 760px;
  }
  .content h2 {
    margin: 0 0 18px 0;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.3px;
    color: var(--text);
  }
  .card {
    background: var(--surface);
    border: 0.5px solid var(--border);
    border-radius: 12px;
    padding: 4px 16px;
    margin-bottom: 16px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 0;
    border-bottom: 0.5px solid var(--border);
  }
  .row:last-child {
    border-bottom: none;
  }
  .row.col {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
  }
  .row-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
  }
  .row-sub {
    font-size: 12px;
    color: var(--text-dim);
    margin-top: 4px;
    line-height: 1.5;
  }
  .row-sub code {
    background: var(--bg);
    padding: 1px 4px;
    border-radius: 4px;
    font-size: 11px;
  }
  .primary {
    padding: 8px 14px;
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 600;
    font-size: 13px;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .input-row {
    display: flex;
    gap: 8px;
  }
  .input-row input {
    flex: 1;
    padding: 8px 12px;
    background: var(--bg);
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    font-size: 13px;
    outline: none;
  }
  .input-row input:focus {
    border-color: var(--accent);
  }
  .ok {
    color: var(--success);
    font-size: 12px;
    margin: 4px 0 0 0;
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    margin: 4px 0 0 0;
  }
  .seg {
    display: flex;
    background: var(--bg);
    border-radius: 6px;
    padding: 2px;
    border: 0.5px solid var(--border);
  }
  .seg button {
    border: 0;
    background: transparent;
    color: var(--text-dim);
    padding: 6px 14px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    font-weight: 500;
  }
  .seg button.active {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .toggle {
    width: 36px;
    height: 20px;
    border-radius: 999px;
    background: var(--border-strong);
    border: 0;
    position: relative;
    cursor: pointer;
  }
  .toggle.on {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.15s;
  }
  .toggle.on .knob {
    transform: translateX(16px);
  }
  .about-logo {
    text-align: center;
    padding: 24px 0 12px;
  }
  .about-name {
    text-align: center;
    font-size: 22px;
    font-weight: 700;
    color: var(--text);
    letter-spacing: -0.4px;
  }
  .about-version {
    text-align: center;
    font-size: 12px;
    color: var(--text-dim);
    font-family: 'JetBrains Mono', monospace;
    margin-top: 4px;
  }
  .about-desc {
    text-align: center;
    color: var(--text-dim);
    line-height: 1.6;
    padding: 16px 24px 8px;
    font-size: 13px;
  }
  .about-links {
    text-align: center;
    padding: 8px 24px;
    font-size: 12px;
  }
  .about-links a {
    color: var(--accent);
    text-decoration: none;
  }
  .about-links a:hover {
    text-decoration: underline;
  }
  .about-links .dot {
    color: var(--text-muted);
    margin: 0 8px;
  }
  .about-license {
    text-align: center;
    color: var(--text-muted);
    font-size: 11px;
    padding: 12px 24px 16px;
  }
  .secondary {
    padding: 8px 14px;
    background: transparent;
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
    font-weight: 500;
    font-size: 13px;
  }
  .secondary:hover {
    background: var(--hover);
  }
  .diag-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 0;
    border-bottom: 0.5px solid var(--border);
    gap: 16px;
  }
  .diag-row:last-child {
    border-bottom: none;
  }
  .diag-row.col {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }
  .diag-label {
    font-size: 12px;
    color: var(--text-dim);
    flex-shrink: 0;
  }
  .diag-value {
    font-size: 13px;
    color: var(--text);
    text-align: right;
    word-break: break-all;
  }
  .diag-value.mono {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
  }
  .diag-addrs {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .diag-addrs code {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    background: var(--bg);
    border: 0.5px solid var(--border);
    border-radius: 6px;
    padding: 6px 10px;
    color: var(--text);
    word-break: break-all;
  }
  .dim {
    color: var(--text-muted);
    font-size: 12px;
  }

  /* Narrow viewport — nav collapses to horizontal tabs */
  @media (max-width: 700px) {
    .body {
      flex-direction: column;
    }
    .nav {
      width: auto;
      flex-direction: row;
      border-right: none;
      border-bottom: 0.5px solid var(--border);
      padding: 8px;
      overflow-x: auto;
    }
    .nav button {
      flex-shrink: 0;
    }
    .content {
      padding: 16px;
    }
  }
</style>
