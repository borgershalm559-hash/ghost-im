<script lang="ts">
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import type { MessageDto } from '$lib/types';

  type Props = {
    msg: MessageDto;
    senderName: string;
  };
  let { msg, senderName }: Props = $props();

  let mine = $derived(msg.direction === 'out');
  let timeText = $derived(
    new Date(msg.sent_at * 1000).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
    })
  );
</script>

<div class="row" class:mine>
  {#if !mine}
    <Avatar name={senderName} size={32} />
  {/if}
  <div class="col" class:mine>
    <div class="bubble" class:mine>{msg.content}</div>
    <div class="meta">
      {timeText}
      {#if mine}
        <span class="check"><Icon name="checkDouble" size={12} sw={2.2} color="var(--accent)" /></span>
      {/if}
    </div>
  </div>
</div>

<style>
  .row {
    display: flex;
    justify-content: flex-start;
    gap: 10px;
    margin: 4px 0;
    padding: 0 24px;
  }
  .row.mine {
    justify-content: flex-end;
  }
  .col {
    max-width: 60%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }
  .col.mine {
    align-items: flex-end;
  }
  .bubble {
    padding: 10px 14px;
    background: var(--bubble);
    border: 0.5px solid var(--border);
    border-radius: 16px;
    border-top-left-radius: 4px;
    color: var(--text);
    font-size: 14px;
    line-height: 1.5;
    letter-spacing: -0.05px;
    white-space: pre-wrap;
    word-wrap: break-word;
  }
  .bubble.mine {
    background: var(--bubble-mine);
    border: none;
    color: #fff;
    border-top-left-radius: 16px;
    border-top-right-radius: 4px;
  }
  .meta {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 4px;
    padding: 0 6px;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .check {
    display: flex;
  }
</style>
