<script lang="ts">
  import { page } from '$app/state';
  import ChatPane from '$lib/components/ChatPane.svelte';

  let contactGhostId = $derived(decodeURIComponent(page.params.ghost_id ?? ''));
</script>

{#key contactGhostId}
  <ChatPane {contactGhostId} />
{/key}
