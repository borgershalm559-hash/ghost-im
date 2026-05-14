<script lang="ts">
  import {
    setPinned as cmdSetPinned,
    setMuted as cmdSetMuted,
    setVerified as cmdSetVerified,
    setRetention as cmdSetRetention,
    listContacts,
  } from '$lib/tauri';
  import { store } from '$lib/state.svelte';
  import { RETENTION_PRESETS, type ContactDto } from '$lib/types';

  type Props = {
    contact: ContactDto;
    x: number;
    y: number;
    onClose: () => void;
  };
  let { contact, x, y, onClose }: Props = $props();

  let busy = $state(false);

  async function refresh() {
    const cs = await listContacts();
    store.setContacts(cs);
  }

  async function togglePin() {
    busy = true;
    try {
      await cmdSetPinned(contact.ghost_id, !contact.pinned);
      await refresh();
      onClose();
    } finally {
      busy = false;
    }
  }
  async function toggleMute() {
    busy = true;
    try {
      await cmdSetMuted(contact.ghost_id, !contact.muted);
      await refresh();
      onClose();
    } finally {
      busy = false;
    }
  }
  async function toggleVerify() {
    busy = true;
    try {
      await cmdSetVerified(contact.ghost_id, !contact.verified);
      await refresh();
      onClose();
    } finally {
      busy = false;
    }
  }
  async function pickRetention(seconds: number | null) {
    busy = true;
    try {
      await cmdSetRetention(contact.ghost_id, seconds);
      await refresh();
      onClose();
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onclick={onClose} oncontextmenu={onClose} />

<div
  class="menu"
  style:left="{x}px"
  style:top="{y}px"
  role="menu"
  onclick={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.preventDefault()}
>
  <button type="button" disabled={busy} onclick={togglePin}>
    {contact.pinned ? 'Открепить' : 'Закрепить'}
  </button>
  <button type="button" disabled={busy} onclick={toggleMute}>
    {contact.muted ? 'Включить уведомления' : 'Выключить уведомления'}
  </button>
  <button type="button" disabled={busy} onclick={toggleVerify}>
    {contact.verified ? 'Снять отметку «проверен»' : 'Отметить как проверенного'}
  </button>
  <div class="divider"></div>
  <div class="label">Исчезающие сообщения</div>
  {#each RETENTION_PRESETS as p}
    <button
      type="button"
      class="preset"
      class:active={contact.retention_seconds === p.seconds}
      disabled={busy}
      onclick={() => pickRetention(p.seconds)}
    >
      {p.label}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 200;
    background: var(--elevated);
    border: 0.5px solid var(--border);
    border-radius: 10px;
    padding: 6px;
    min-width: 220px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    color: var(--text);
    animation: scale-up 0.14s cubic-bezier(0.16, 1, 0.3, 1);
    transform-origin: top left;
  }
  .menu button {
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    color: var(--text);
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
    transition: background-color 0.12s;
  }
  .menu button:hover {
    background: var(--hover);
  }
  .menu .preset.active {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .divider {
    height: 0.5px;
    background: var(--border);
    margin: 4px 0;
  }
  .label {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.6px;
    text-transform: uppercase;
    padding: 6px 10px 2px;
  }
</style>
