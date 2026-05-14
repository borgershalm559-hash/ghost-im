<script lang="ts">
  import Icon from './Icon.svelte';

  type Props = {
    onSend: (text: string) => Promise<void>;
    disabled?: boolean;
  };
  let { onSend, disabled = false }: Props = $props();

  let text = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let textarea: HTMLTextAreaElement | null = $state(null);

  async function send() {
    const t = text.trim();
    if (t === '' || busy) return;
    busy = true;
    errorMsg = null;
    try {
      await onSend(t);
      text = '';
      if (textarea) textarea.style.height = 'auto';
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  }

  function autosize() {
    if (!textarea) return;
    textarea.style.height = 'auto';
    textarea.style.height = Math.min(textarea.scrollHeight, 160) + 'px';
  }
</script>

<div class="wrap">
  <div class="bar">
    <textarea
      bind:this={textarea}
      bind:value={text}
      onkeydown={onKey}
      oninput={autosize}
      disabled={disabled || busy}
      rows="1"
      placeholder="Напишите сообщение…"
    ></textarea>
    <button
      type="button"
      class="send"
      onclick={send}
      disabled={disabled || busy || text.trim() === ''}
      aria-label="Отправить"
    >
      <Icon name="send" size={16} sw={2} color="#fff" />
    </button>
  </div>
  {#if errorMsg}<p class="error">{errorMsg}</p>{/if}
</div>

<style>
  .wrap {
    padding: 12px 20px 16px;
    border-top: 0.5px solid var(--border);
    background: var(--bg);
  }
  .bar {
    background: var(--surface);
    border: 0.5px solid var(--border);
    border-radius: 14px;
    padding: 4px 6px 4px 14px;
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }
  textarea {
    flex: 1;
    border: 0;
    background: transparent;
    color: var(--text);
    font-size: 14px;
    line-height: 1.5;
    padding: 9px 0;
    resize: none;
    outline: none;
    font-family: inherit;
    max-height: 160px;
  }
  textarea::placeholder {
    color: var(--text-muted);
  }
  .send {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    border: 0;
    cursor: pointer;
    background: linear-gradient(135deg, #6c5ce7, var(--accent));
    color: #fff;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 16px var(--accent-soft);
    transition:
      transform 0.12s ease,
      box-shadow 0.18s ease,
      filter 0.18s ease,
      opacity 0.18s ease;
  }
  .send:hover:not(:disabled) {
    transform: scale(1.05);
    box-shadow: 0 6px 20px var(--accent-soft);
    filter: brightness(1.1);
  }
  .send:active:not(:disabled) {
    transform: scale(0.95);
  }
  .send:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error {
    margin: 8px 0 0 0;
    color: var(--danger);
    font-size: 12px;
  }
</style>
