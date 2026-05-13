<script lang="ts">
  import Icon from './Icon.svelte';
</script>

<div class="banner">
  <Icon name="lock" size={13} sw={2.2} color="var(--accent)" />
  Сообщения end-to-end зашифрованы. Только вы и собеседник можете их прочитать.
</div>

<style>
  .banner {
    margin: 20px auto;
    padding: 8px 14px;
    max-width: fit-content;
    background: var(--accent-dim);
    border-radius: 999px;
    font-size: 12px;
    color: var(--accent);
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
