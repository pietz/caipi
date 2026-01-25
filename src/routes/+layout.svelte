<script lang="ts">
  import '../app.css';
  import { resolvedTheme, applyTheme } from '$lib/stores';

  let { children } = $props();

  // Apply theme whenever it changes
  $effect(() => {
    const unsubscribe = resolvedTheme.subscribe(theme => {
      applyTheme(theme);
    });

    return unsubscribe;
  });
</script>

<div class="h-screen w-screen overflow-hidden bg-background text-foreground">
  {@render children()}
</div>
