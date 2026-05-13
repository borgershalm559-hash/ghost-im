<script lang="ts">
  import Modal from './Modal.svelte';
  import { store } from '$lib/state.svelte';

  type Props = {
    open: boolean;
    onClose: () => void;
  };
  let { open, onClose }: Props = $props();

  let copiedId = $state(false);
  let copiedFp = $state(false);

  async function copyId() {
    if (!store.info) return;
    await navigator.clipboard.writeText(store.info.ghost_id);
    copiedId = true;
    setTimeout(() => (copiedId = false), 1500);
  }

  async function copyFp() {
    if (!store.info) return;
    await navigator.clipboard.writeText(store.info.fingerprint);
    copiedFp = true;
    setTimeout(() => (copiedFp = false), 1500);
  }
</script>

<Modal {open} {onClose} title="Ваш Ghost ID">
  <p class="desc">
    Поделитесь полным ID или коротким fingerprint'ом для verbal-сверки. ID не
    содержит секретов — это публичный ключ вашей идентификации.
  </p>

  {#if store.info}
    <div class="label">Полный ID</div>
    <div class="row">
      <code>{store.info.ghost_id}</code>
      <button type="button" onclick={copyId}>{copiedId ? '✓' : 'Копировать'}</button>
    </div>

    <div class="label">Fingerprint</div>
    <div class="row">
      <code class="fp">{store.info.fingerprint}</code>
      <button type="button" onclick={copyFp}>{copiedFp ? '✓' : 'Копировать'}</button>
    </div>
  {/if}
</Modal>

<style>
  .desc {
    margin: 0 0 14px 0;
    color: var(--text-dim);
    font-size: 13px;
  }
  .label {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.6px;
    text-transform: uppercase;
    margin-top: 12px;
    margin-bottom: 6px;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    background: var(--bg);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
  }
  code {
    flex: 1;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    color: var(--text);
    word-break: break-all;
    overflow-wrap: anywhere;
  }
  code.fp {
    font-size: 14px;
    letter-spacing: 0.5px;
  }
  button {
    padding: 6px 12px;
    background: transparent;
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
    flex-shrink: 0;
  }
  button:hover {
    background: var(--hover);
  }
</style>
