<script lang="ts">
  import Modal from './Modal.svelte';
  import { createInvite } from '$lib/tauri';

  type Props = {
    open: boolean;
    onClose: () => void;
  };
  let { open, onClose }: Props = $props();

  let invite = $state<string | null>(null);
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let copied = $state(false);

  async function generate() {
    busy = true;
    errorMsg = null;
    copied = false;
    try {
      const r = await createInvite();
      invite = r.bech32;
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  async function copy() {
    if (!invite) return;
    try {
      await navigator.clipboard.writeText(invite);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (e) {
      errorMsg = String(e);
    }
  }

  $effect(() => {
    if (!open) {
      invite = null;
      copied = false;
      errorMsg = null;
    }
  });
</script>

<Modal {open} {onClose} title="Создать инвайт">
  <p class="desc">Поделитесь этой строкой с одним человеком. Срок действия — 7 дней.</p>

  {#if !invite}
    <button type="button" class="primary" onclick={generate} disabled={busy}>
      {busy ? 'Генерация…' : 'Создать инвайт'}
    </button>
  {:else}
    <textarea readonly rows="3" class="bech">{invite}</textarea>
    <button type="button" class="ghost" onclick={copy}>
      {copied ? 'Скопировано!' : 'Копировать'}
    </button>
  {/if}

  {#if errorMsg}
    <p class="error">{errorMsg}</p>
  {/if}
</Modal>

<style>
  .desc {
    margin: 0 0 12px 0;
    color: var(--text-dim);
    font-size: 13px;
  }
  .primary {
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 600;
  }
  .ghost {
    margin-top: 8px;
    padding: 8px 14px;
    background: transparent;
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
  }
  .bech {
    width: 100%;
    padding: 10px;
    background: var(--bg);
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    resize: none;
  }
  .error {
    color: var(--danger);
    font-size: 12px;
    margin-top: 8px;
  }
</style>
