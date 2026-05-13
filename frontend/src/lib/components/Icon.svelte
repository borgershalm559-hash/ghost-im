<script lang="ts">
  import { I, type IconName } from '$lib/icons';

  type Props = {
    name: IconName;
    size?: number;
    sw?: number;
    color?: string;
  };

  let { name, size = 16, sw, color = 'currentColor' }: Props = $props();
  let def = $derived(I[name]);
  let strokeWidth = $derived(sw ?? def.sw);
  // SVG can render multiple subpaths in a single <path d="..." />, so we just
  // pass `def.d` through. Caller-facing colour: fill if def.fill, else stroke.
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={def.fill ? color : 'none'}
  stroke={def.fill ? 'none' : color}
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d={def.d} />
</svg>
