<script lang="ts">
  import UpdateBanner from '$lib/components/UpdateBanner.svelte';

  let { children } = $props();
</script>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: #0e0f12;
    color: #e8e8ec;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  }
  :global(*) {
    box-sizing: border-box;
  }
  main {
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
  }
  .layout-content {
    flex: 1;
    overflow: auto;
  }
</style>

<main>
  <UpdateBanner />
  <div class="layout-content">
    {@render children()}
  </div>
</main>
