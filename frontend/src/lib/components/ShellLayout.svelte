<script lang="ts">
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
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

  // On narrow viewports we show ONE of {chat list, chat pane} at a time.
  // Rule: if a chat is open (URL has /chat/...) → show pane. Otherwise → list.
  // Wide viewports show both. The `compact` class toggles the layout via CSS.
  let hasChatRoute = $derived(page.url.pathname.startsWith('/chat/'));
</script>

<div class="shell" class:chat-open={hasChatRoute}>
  <Rail onProfileClick={() => (popoverOpen = !popoverOpen)} />
  <div class="list-wrap">
    <ChatList />
  </div>
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
    min-height: 0;
  }
  .list-wrap {
    display: contents;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  /* Narrow viewport (<900px width): hide whichever pane the user isn't
     focused on. On the home route (no chat open) show only rail + list.
     On a /chat/[id] route show only rail + chat pane. */
  @media (max-width: 900px) {
    .shell.chat-open .list-wrap {
      display: none;
    }
    .shell:not(.chat-open) .main {
      display: none;
    }
    /* When chat list is the visible pane, let it grow to fill the rest. */
    .shell:not(.chat-open) :global(.chat-list) {
      flex: 1;
      width: auto;
    }
  }

  /* Very narrow (<480px): collapse rail to icon-only or hide labels.
     Tauri minWidth keeps us at 800+, but if the user manually resizes
     below that we still want something usable. */
  @media (max-width: 480px) {
    .shell :global(.rail) {
      width: 60px;
    }
    .shell :global(.rail .label) {
      display: none;
    }
  }
</style>
