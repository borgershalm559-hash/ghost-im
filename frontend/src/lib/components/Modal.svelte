<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    open: boolean;
    onClose: () => void;
    title: string;
    children: Snippet;
  };

  let { open, onClose, title, children }: Props = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <div
    class="backdrop"
    role="dialog"
    aria-modal="true"
    aria-label={title}
    onclick={(e) => {
      if (e.currentTarget === e.target) onClose();
    }}
  >
    <div class="pane">
      <header>
        <h2>{title}</h2>
        <button type="button" class="close" onclick={onClose} aria-label="Закрыть">×</button>
      </header>
      <div class="body">
        {@render children()}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }
  .pane {
    background: var(--surface);
    border: 0.5px solid var(--border);
    border-radius: 14px;
    width: min(560px, 90vw);
    max-height: 80vh;
    overflow: auto;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.5);
  }
  header {
    display: flex;
    align-items: center;
    padding: 14px 18px;
    border-bottom: 0.5px solid var(--border);
  }
  h2 {
    margin: 0;
    flex: 1;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.2px;
    color: var(--text);
  }
  .close {
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    font-size: 22px;
    line-height: 1;
    border-radius: 6px;
  }
  .close:hover {
    background: var(--hover);
    color: var(--text);
  }
  .body {
    padding: 18px;
    color: var(--text);
  }
</style>
