<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { theme, applyTheme } from '$lib/stores';
  import { updater } from '$lib/stores/updater.svelte';
  import UpdateBanner from '$lib/components/ui/UpdateBanner.svelte';

  let { children } = $props();

  // Apply theme whenever it changes
  $effect(() => {
    applyTheme(theme.resolved);
  });

  // Check for updates on startup (with a delay to not block initial load)
  onMount(() => {
    const timer = setTimeout(() => {
      updater.checkForUpdates(true);
    }, 5000);

    return () => clearTimeout(timer);
  });
</script>

<div class="h-screen w-screen overflow-hidden bg-background text-foreground">
  {@render children()}
</div>

<UpdateBanner />
