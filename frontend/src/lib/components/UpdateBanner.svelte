<script lang="ts">
  import { onMount } from 'svelte';
  import {
    checkForUpdate,
    downloadAndInstallUpdate,
    onUpdateProgress
  } from '$lib/tauri';
  import type { UpdateAvailableDto, UpdateProgress } from '$lib/tauri';

  let update = $state<UpdateAvailableDto | null>(null);
  let dismissed = $state(false);
  let downloading = $state(false);
  let progress = $state<UpdateProgress | null>(null);
  let errorMsg = $state<string | null>(null);
  let unlistenProgress: (() => void) | null = null;
  let pollHandle: ReturnType<typeof setInterval> | null = null;

  const POLL_MS = 60 * 60 * 1000; // 1 hour

  async function poll() {
    try {
      const result = await checkForUpdate();
      if (result) {
        update = result;
      }
    } catch (e) {
      // Silent for poll failures; logged via console for debugging.
      console.warn('update check failed:', e);
    }
  }

  async function restart() {
    if (!update || downloading) return;
    downloading = true;
    errorMsg = null;
    try {
      await downloadAndInstallUpdate();
      // Process exits via installer; we won't reach here normally.
    } catch (e) {
      errorMsg = String(e);
      downloading = false;
    }
  }

  function dismiss() {
    dismissed = true;
  }

  onMount(() => {
    void poll();
    pollHandle = setInterval(poll, POLL_MS);

    void onUpdateProgress((p) => {
      progress = p;
    }).then((u) => {
      unlistenProgress = u;
    });

    return () => {
      if (pollHandle) clearInterval(pollHandle);
      unlistenProgress?.();
    };
  });

  let visible = $derived(update !== null && !dismissed);
</script>

{#if visible && update}
  <div
    class="update-banner"
    style="position: sticky; top: 0; left: 0; right: 0; background: #3a2e15; color: #f5d486; padding: 0.6rem 1rem; display: flex; align-items: center; gap: 1rem; border-bottom: 1px solid #5a4625; font-size: 0.9rem; z-index: 100;"
    role="status"
    aria-live="polite"
  >
    {#if downloading}
      <span style="flex: 1;">
        Скачивается обновление…
        {#if progress && progress.total > 0}
          {Math.round((progress.chunk / progress.total) * 100)}%
        {/if}
      </span>
    {:else}
      <span style="flex: 1;">↑ Доступна Ghost {update.version}</span>
      <button
        type="button"
        onclick={restart}
        style="padding: 0.3rem 0.8rem; background: #f5d486; color: #1a1c22; border: 0; border-radius: 4px; cursor: pointer; font-weight: 500;"
      >
        Перезапустить
      </button>
      <button
        type="button"
        onclick={dismiss}
        style="padding: 0.3rem 0.8rem; background: transparent; color: inherit; border: 1px solid #5a4625; border-radius: 4px; cursor: pointer;"
      >
        Позже
      </button>
    {/if}

    {#if errorMsg}
      <span style="color: #ff6464; margin-left: 0.5rem;">{errorMsg}</span>
    {/if}
  </div>
{/if}

<style>
  .update-banner {
    animation: slide-down 0.24s cubic-bezier(0.16, 1, 0.3, 1);
  }
  .update-banner button {
    transition:
      transform 0.1s ease,
      filter 0.15s ease;
  }
  .update-banner button:hover {
    filter: brightness(1.06);
  }
  .update-banner button:active {
    transform: scale(0.97);
  }
</style>
