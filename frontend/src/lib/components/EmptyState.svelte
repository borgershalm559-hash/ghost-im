<script lang="ts">
  import Icon from './Icon.svelte';
  import InviteModal from './InviteModal.svelte';
  import AddContactModal from './AddContactModal.svelte';

  let inviteOpen = $state(false);
  let addOpen = $state(false);
</script>

<div class="root">
  <div class="bg"></div>
  <div class="ghost">
    {#each [120, 90, 60] as r, i}
      <div
        class="orbit"
        style:width="{r * 2}px"
        style:height="{r * 2}px"
        style:margin-left="-{r}px"
        style:margin-top="-{r}px"
        style:opacity={(0.6 - i * 0.15).toFixed(2)}
      ></div>
    {/each}
    <svg width="280" height="280" viewBox="0 0 280 280" style="position: relative; z-index: 1;">
      <defs>
        <linearGradient id="ghostGrad" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="var(--accent)" stop-opacity="0.95" />
          <stop offset="100%" stop-color="#6c5ce7" stop-opacity="0.85" />
        </linearGradient>
        <filter id="softGlow">
          <feGaussianBlur stdDeviation="6" />
        </filter>
      </defs>
      <g transform="translate(70, 60)">
        <path
          d="M70 0 C30 0, 0 32, 0 72 L0 158 L18 144 L36 158 L54 144 L72 158 L90 144 L108 158 L126 144 L140 158 L140 72 C140 32, 110 0, 70 0 Z"
          fill="url(#ghostGrad)"
          opacity="0.18"
          filter="url(#softGlow)"
        />
        <path
          d="M70 0 C30 0, 0 32, 0 72 L0 158 L18 144 L36 158 L54 144 L72 158 L90 144 L108 158 L126 144 L140 158 L140 72 C140 32, 110 0, 70 0 Z"
          fill="url(#ghostGrad)"
        />
        <ellipse cx="48" cy="68" rx="6" ry="8" fill="var(--bg)" />
        <ellipse cx="92" cy="68" rx="6" ry="8" fill="var(--bg)" />
        <ellipse cx="48" cy="66" rx="2" ry="2.5" fill="#fff" opacity="0.6" />
        <ellipse cx="92" cy="66" rx="2" ry="2.5" fill="#fff" opacity="0.6" />
        <path
          d="M58 96 Q70 104 82 96"
          stroke="var(--bg)"
          stroke-width="3"
          stroke-linecap="round"
          fill="none"
          opacity="0.7"
        />
      </g>
      <circle cx="40" cy="70" r="3" fill="var(--accent)" opacity="0.5" />
      <circle cx="240" cy="100" r="2" fill="var(--accent)" opacity="0.4" />
      <circle cx="220" cy="220" r="4" fill="var(--accent)" opacity="0.3" />
      <circle cx="50" cy="220" r="2.5" fill="var(--accent)" opacity="0.4" />
    </svg>
  </div>

  <div class="title">Выберите чат, чтобы начать беседу</div>
  <div class="sub">
    Каждое сообщение в Ghost зашифровано end-to-end и не оставляет следов на серверах.
  </div>

  <div class="pills">
    <span class="pill"><Icon name="lock" size={13} sw={2} color="var(--success)" /> E2E активно</span>
    <span class="pill"><Icon name="ghost" size={13} sw={2} color="var(--text-dim)" /> 0 логов</span>
  </div>

  <div class="cta">
    <button type="button" class="primary" onclick={() => (inviteOpen = true)}>Создать инвайт</button>
    <button type="button" class="ghost-btn" onclick={() => (addOpen = true)}>Добавить контакт</button>
  </div>
</div>

<InviteModal open={inviteOpen} onClose={() => (inviteOpen = false)} />
<AddContactModal open={addOpen} onClose={() => (addOpen = false)} />

<style>
  .root {
    flex: 1;
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    position: relative;
    overflow: hidden;
  }
  .bg {
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at 50% 40%, var(--accent-dim) 0%, transparent 55%);
    pointer-events: none;
  }
  .ghost {
    position: relative;
    width: 280px;
    height: 280px;
    margin-bottom: 32px;
    animation: ghost-float 5s ease-in-out infinite;
  }
  .orbit {
    position: absolute;
    left: 50%;
    top: 50%;
    border-radius: 50%;
    border: 1px dashed var(--border);
    animation: orbit-pulse 4s ease-in-out infinite;
  }
  .orbit:nth-child(1) {
    animation-delay: 0s;
  }
  .orbit:nth-child(2) {
    animation-delay: -1.3s;
  }
  .orbit:nth-child(3) {
    animation-delay: -2.6s;
  }
  .title,
  .sub,
  .pills,
  .cta {
    animation: slide-up 0.4s 0.1s ease-out backwards;
  }
  .sub {
    animation-delay: 0.18s;
  }
  .pills {
    animation-delay: 0.26s;
  }
  .cta {
    animation-delay: 0.34s;
  }
  .title {
    font-size: 22px;
    font-weight: 600;
    color: var(--text);
    letter-spacing: -0.4px;
    margin-bottom: 8px;
    position: relative;
    z-index: 1;
  }
  .sub {
    font-size: 14px;
    color: var(--text-dim);
    max-width: 380px;
    text-align: center;
    line-height: 1.6;
    position: relative;
    z-index: 1;
  }
  .pills {
    display: flex;
    gap: 8px;
    margin-top: 28px;
    position: relative;
    z-index: 1;
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-radius: 999px;
    background: var(--surface);
    border: 0.5px solid var(--border);
    font-size: 12px;
    font-weight: 500;
    color: var(--text-dim);
  }
  .cta {
    display: flex;
    gap: 8px;
    margin-top: 36px;
    position: relative;
    z-index: 1;
  }
  .primary {
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 10px;
    cursor: pointer;
    font-weight: 600;
    font-size: 13px;
    transition:
      transform 0.12s ease,
      box-shadow 0.18s ease,
      filter 0.18s ease;
  }
  .primary:hover {
    transform: translateY(-1px);
    box-shadow: 0 6px 20px var(--accent-soft);
    filter: brightness(1.05);
  }
  .primary:active {
    transform: translateY(0);
  }
  .ghost-btn {
    padding: 10px 18px;
    background: transparent;
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 10px;
    cursor: pointer;
    font-weight: 500;
    font-size: 13px;
  }
</style>
