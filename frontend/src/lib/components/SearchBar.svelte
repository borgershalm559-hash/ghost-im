<script lang="ts">
  import Icon from './Icon.svelte';

  type Props = {
    value: string;
    placeholder?: string;
    onInput: (v: string) => void;
  };

  let { value, placeholder = 'Поиск · ⌘K', onInput }: Props = $props();
</script>

<div class="wrap">
  <div class="box">
    <span class="ic"><Icon name="search" size={14} color="var(--text-muted)" /></span>
    <input
      type="text"
      {value}
      {placeholder}
      oninput={(e) => onInput((e.currentTarget as HTMLInputElement).value)}
    />
  </div>
</div>

<style>
  .wrap {
    padding: 0 12px 10px;
  }
  .box {
    height: 36px;
    border-radius: 10px;
    background: var(--surface);
    border: 0.5px solid var(--border);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
  }
  .ic {
    display: flex;
  }
  input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text);
    font-size: 13px;
    outline: none;
  }
  input::placeholder {
    color: var(--text-muted);
  }
</style>
