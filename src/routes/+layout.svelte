<script lang="ts">
  import '../app.css';
  import { api } from '$lib/api';
  import { onMount } from 'svelte';
  import { theme, applyTheme } from '$lib/stores';
  import { updater } from '$lib/stores/updater.svelte';
  import UpdateBanner from '$lib/components/ui/UpdateBanner.svelte';
  import { isMacOS } from '$lib/utils/platform';

  let { children } = $props();

  // Apply theme whenever it changes
  $effect(() => {
    applyTheme(theme.resolved);
  });

  // Check for updates on startup (with a delay to not block initial load)
  onMount(() => {
    function handleKeydown(e: KeyboardEvent) {
      const hasModifier = isMacOS() ? e.metaKey : e.ctrlKey;
      if (!hasModifier || e.key.toLowerCase() !== 'n' || e.repeat) return;

      const target = e.target as HTMLElement | null;
      const tagName = target?.tagName;
      const isEditable =
        target?.isContentEditable ||
        tagName === 'INPUT' ||
        tagName === 'TEXTAREA' ||
        tagName === 'SELECT';
      if (isEditable) return;

      e.preventDefault();
      void api.createWindow().catch((err) => {
        console.error('Failed to create window:', err);
      });
    }

    const timer = setTimeout(() => {
      updater.checkForUpdates(true);
    }, 3000);
    window.addEventListener('keydown', handleKeydown);

    return () => {
      clearTimeout(timer);
      window.removeEventListener('keydown', handleKeydown);
    };
  });
</script>

<div class="h-screen w-screen overflow-hidden bg-background text-foreground">
  {@render children()}
</div>

<UpdateBanner />
