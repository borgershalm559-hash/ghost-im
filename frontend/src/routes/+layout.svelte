<script lang="ts">
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
  }
</style>

<main>
  {@render children()}
</main>
