<script lang="ts">
  import type { Snippet } from 'svelte';
  import Rail from './Rail.svelte';
  import ChatList from './ChatList.svelte';
  import ProfilePopover from './ProfilePopover.svelte';
  import IdentityModal from './IdentityModal.svelte';

  type Props = {
    children: Snippet;
  };
  let { children }: Props = $props();

  let popoverOpen = $state(false);
  let identityOpen = $state(false);
</script>

<div class="shell">
  <Rail onProfileClick={() => (popoverOpen = !popoverOpen)} />
  <ChatList />
  <main class="main">{@render children()}</main>
</div>

<ProfilePopover
  open={popoverOpen}
  onClose={() => (popoverOpen = false)}
  onShowIdentity={() => {
    popoverOpen = false;
    identityOpen = true;
  }}
/>

<IdentityModal open={identityOpen} onClose={() => (identityOpen = false)} />

<style>
  .shell {
    display: flex;
    height: 100%;
    background: var(--bg);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
</style>
