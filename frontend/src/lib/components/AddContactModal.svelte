<script lang="ts">
  import Modal from './Modal.svelte';
  import { addContact, listContacts } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  type Props = {
    open: boolean;
    onClose: () => void;
  };
  let { open, onClose }: Props = $props();

  let inviteInput = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let okMsg = $state<string | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    if (inviteInput.trim() === '' || busy) return;
    busy = true;
    errorMsg = null;
    okMsg = null;
    try {
      await addContact(inviteInput.trim());
      inviteInput = '';
      okMsg = 'Контакт добавлен.';
      const cs = await listContacts();
      store.setContacts(cs);
      setTimeout(() => onClose(), 1000);
    } catch (e) {
      const raw = String(e);
      if (/cannot add yourself/i.test(raw)) {
        errorMsg = 'Нельзя добавить себя как контакт — это ваш собственный инвайт.';
      } else if (
        /Не удалось соединиться|PeerUnreachable|outbound failure|Failed to dial|Timeout/i.test(raw)
      ) {
        errorMsg =
          'Не удалось дозвониться до собеседника. Убедитесь, что у него запущен Ghost и вы в одной сети либо у него публичный IP. В MVP-1 нужно прямое соединение или общий DHT-bootstrap.';
      } else if (/invite expired/i.test(raw)) {
        errorMsg = 'Инвайт просрочен. Попросите собеседника создать новый.';
      } else if (/invite signature invalid|invite parse/i.test(raw)) {
        errorMsg = 'Инвайт повреждён или неправильно скопирован.';
      } else {
        errorMsg = raw;
      }
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (!open) {
      inviteInput = '';
      errorMsg = null;
      okMsg = null;
    }
  });
</script>

<Modal {open} {onClose} title="Добавить контакт">
  <p class="desc">Вставьте инвайт-строку, которую вам прислали.</p>

  <form onsubmit={submit}>
    <textarea
      bind:value={inviteInput}
      disabled={busy}
      rows="3"
      placeholder="ghostinvite1q…"
    ></textarea>
    <button
      type="submit"
      class="primary"
      disabled={busy || inviteInput.trim() === ''}
    >
      {busy ? 'Добавление…' : 'Добавить'}
    </button>
  </form>

  {#if errorMsg}<p class="error">{errorMsg}</p>{/if}
  {#if okMsg}<p class="ok">{okMsg}</p>{/if}
</Modal>

<style>
  .desc {
    margin: 0 0 12px 0;
    color: var(--text-dim);
    font-size: 13px;
  }
  textarea {
    width: 100%;
    padding: 10px;
    background: var(--bg);
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    resize: vertical;
  }
  .primary {
    margin-top: 10px;
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 600;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error {
    color: var(--danger);
    font-size: 12px;
    margin-top: 8px;
  }
  .ok {
    color: var(--success);
    font-size: 12px;
    margin-top: 8px;
  }
</style>
