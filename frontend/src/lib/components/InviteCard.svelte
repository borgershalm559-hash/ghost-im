<script lang="ts">
  import { createInvite } from '$lib/tauri';

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
</script>

<div style="border: 1px solid #2a2d36; border-radius: 8px; padding: 1rem; margin-bottom: 1rem;">
  <h3 style="margin: 0 0 0.5rem 0;">Your invite</h3>
  <p style="opacity: 0.7; font-size: 0.9rem; margin: 0 0 0.75rem 0;">
    Share this string with one person. It expires in 7 days.
  </p>
  <button
    type="button"
    onclick={generate}
    disabled={busy}
    style="padding: 0.5rem 1rem; background: #4a4cff; color: white; border: 0; border-radius: 6px; cursor: pointer;"
  >
    {busy ? 'Generating…' : 'Generate invite'}
  </button>

  {#if invite}
    <div style="margin-top: 0.75rem;">
      <textarea
        readonly
        rows="3"
        style="width: 100%; padding: 0.5rem; background: #14151a; color: inherit; border: 1px solid #2a2d36; border-radius: 6px; font-family: monospace; font-size: 0.85rem;"
      >{invite}</textarea>
      <button
        type="button"
        onclick={copy}
        style="margin-top: 0.5rem; padding: 0.4rem 0.8rem; background: transparent; color: inherit; border: 1px solid #2a2d36; border-radius: 6px; cursor: pointer;"
      >
        {copied ? 'Copied!' : 'Copy'}
      </button>
    </div>
  {/if}

  {#if errorMsg}
    <p style="color: #ff6464; margin: 0.5rem 0 0 0;">{errorMsg}</p>
  {/if}
</div>
