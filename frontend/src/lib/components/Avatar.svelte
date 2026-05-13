<script lang="ts">
  type Props = {
    name: string;
    size?: number;
    online?: boolean;
    ghost?: boolean;
    square?: boolean;
  };

  let { name, size = 40, online = false, ghost = false, square = false }: Props = $props();

  let seed = $derived(name.charCodeAt(0) + (name.charCodeAt(1) || 0));
  let hue = $derived((seed * 37) % 360);
  let grad = $derived(
    `linear-gradient(135deg, oklch(0.65 0.18 ${hue}) 0%, oklch(0.55 0.20 ${(hue + 40) % 360}) 100%)`
  );
  let initials = $derived(
    name
      .split(' ')
      .map((s) => s[0] ?? '')
      .slice(0, 2)
      .join('')
      .toUpperCase() || '?'
  );
</script>

<div class="root" style:width="{size}px" style:height="{size}px">
  <div
    class="bubble"
    style:width="{size}px"
    style:height="{size}px"
    style:border-radius={square ? `${size * 0.28}px` : '50%'}
    style:background={grad}
    style:font-size="{size * 0.4}px"
    style:box-shadow={ghost ? `0 0 0 2px var(--bg), 0 0 0 3.5px var(--accent)` : 'none'}
  >
    {initials}
  </div>
  {#if online}
    <div
      class="dot online"
      style:width="{size * 0.28}px"
      style:height="{size * 0.28}px"
    ></div>
  {/if}
  {#if ghost}
    <div
      class="dot ghost"
      style:width="{size * 0.32}px"
      style:height="{size * 0.32}px"
    >
      <svg width={size * 0.18} height={size * 0.18} viewBox="0 0 24 24" fill="#fff">
        <path d="M12 2a8 8 0 0 0-8 8v11l3-2 3 2 2-2 2 2 3-2 3 2V10a8 8 0 0 0-8-8Z" />
      </svg>
    </div>
  {/if}
</div>

<style>
  .root {
    position: relative;
    flex-shrink: 0;
  }
  .bubble {
    color: #fff;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    letter-spacing: -0.3px;
  }
  .dot {
    position: absolute;
    border-radius: 50%;
  }
  .dot.online {
    bottom: 0;
    right: 0;
    background: var(--success);
    border: 2px solid var(--sidebar);
  }
  .dot.ghost {
    bottom: -1px;
    right: -1px;
    background: var(--accent);
    border: 2px solid var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
