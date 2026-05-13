<script lang="ts">
  import { goto } from '$app/navigation';
  import Icon from './Icon.svelte';
  import Avatar from './Avatar.svelte';
  import { store } from '$lib/state.svelte';
  import type { IconName } from '$lib/icons';

  type Folder = {
    id: string;
    icon: IconName;
    label: string;
    /** Decorative — click is a no-op. */
    decorative?: boolean;
  };

  const FOLDERS: Folder[] = [
    { id: 'all', icon: 'inbox', label: 'Все' },
    { id: 'personal', icon: 'user', label: 'Личные', decorative: true },
    { id: 'work', icon: 'users', label: 'Работа', decorative: true },
    { id: 'crypto', icon: 'key', label: 'Crypto', decorative: true },
    { id: 'channels', icon: 'hash', label: 'Каналы', decorative: true },
    { id: 'burner', icon: 'ghost', label: 'Burner', decorative: true },
    { id: 'archive', icon: 'archive', label: 'Архив', decorative: true },
  ];

  type Props = {
    onProfileClick: () => void;
  };
  let { onProfileClick }: Props = $props();

  let avatarName = $derived(store.info?.fingerprint ?? '?');

  function clickFolder(f: Folder) {
    if (f.decorative) return;
    // "All" navigates to home (which shows chat list on narrow viewports,
    // empty state on wide ones).
    if (f.id === 'all') void goto('/');
  }
</script>

<aside class="rail">
  {#each FOLDERS as f (f.id)}
    <button
      type="button"
      class="cell"
      class:active={f.id === 'all'}
      class:decorative={f.decorative}
      disabled={f.decorative}
      onclick={() => clickFolder(f)}
      title={f.decorative ? 'Папки появятся в следующих версиях' : f.label}
    >
      <span class="ic">
        <Icon
          name={f.icon}
          size={22}
          sw={1.8}
          color={f.id === 'all' ? 'var(--accent)' : 'var(--text-dim)'}
        />
      </span>
      <span class="label">{f.label}</span>
    </button>
  {/each}

  <div class="spacer"></div>

  <button type="button" class="profile" onclick={onProfileClick} aria-label="Профиль">
    <Avatar name={avatarName} size={40} ghost={store.ghostMode} />
  </button>
</aside>

<style>
  .rail {
    width: 80px;
    background: var(--rail);
    border-right: 0.5px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: stretch;
    padding: 12px 6px;
    gap: 4px;
    flex-shrink: 0;
  }
  .cell {
    padding: 10px 6px;
    border-radius: 10px;
    background: transparent;
    border: 0;
    cursor: pointer;
    color: var(--text-dim);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    font: inherit;
  }
  .cell.active {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .cell.decorative {
    cursor: default;
    opacity: 0.5;
  }
  .ic {
    display: flex;
  }
  .label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1px;
  }
  .spacer {
    flex: 1;
  }
  .profile {
    background: transparent;
    border: 0;
    padding: 0;
    cursor: pointer;
    align-self: center;
  }
</style>
