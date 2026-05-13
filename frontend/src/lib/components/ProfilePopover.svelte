<script lang="ts">
  import { store } from '$lib/state.svelte';
  import { persistTheme } from '$lib/theme';
  import { setSetting } from '$lib/tauri';

  type Props = {
    open: boolean;
    onClose: () => void;
    onShowIdentity: () => void;
  };
  let { open, onClose, onShowIdentity }: Props = $props();

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
      // settings unavailable (e.g. client not open yet) — keep visual state
    }
  }
</script>

{#if open}
  <div
    class="popover"
    role="dialog"
    aria-label="Настройки профиля"
    onmouseleave={onClose}
  >
    <div class="row">
      <div class="label">Тема</div>
      <div class="seg">
        <button
          type="button"
          class:active={store.theme === 'dark'}
          onclick={() => pickTheme('dark')}>Тёмная</button
        >
        <button
          type="button"
          class:active={store.theme === 'light'}
          onclick={() => pickTheme('light')}>Светлая</button
        >
      </div>
    </div>

    <div class="row">
      <div class="label">Ghost mode</div>
      <button type="button" class="toggle" class:on={store.ghostMode} onclick={toggleGhostMode}>
        <span class="knob"></span>
      </button>
    </div>

    <div class="divider"></div>

    <button type="button" class="action" onclick={onShowIdentity}>Показать мой Ghost ID</button>
  </div>
{/if}

<style>
  .popover {
    position: fixed;
    bottom: 76px;
    left: 88px;
    width: 240px;
    background: var(--elevated);
    border: 0.5px solid var(--border);
    border-radius: 12px;
    padding: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    z-index: 100;
    color: var(--text);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 6px;
  }
  .label {
    font-size: 12px;
    color: var(--text-dim);
    font-weight: 500;
  }
  .seg {
    display: flex;
    background: var(--surface);
    border-radius: 6px;
    padding: 2px;
    border: 0.5px solid var(--border);
  }
  .seg button {
    border: 0;
    background: transparent;
    color: var(--text-dim);
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
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
  .divider {
    height: 0.5px;
    background: var(--border);
    margin: 6px 0;
  }
  .action {
    width: 100%;
    padding: 8px;
    border: 0;
    background: transparent;
    color: var(--text);
    text-align: left;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }
  .action:hover {
    background: var(--hover);
  }
</style>
