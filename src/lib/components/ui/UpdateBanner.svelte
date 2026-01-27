<script lang="ts">
  import { updater } from '$lib/stores/updater.svelte';
  import { Download, RefreshCw, X, Loader2, CheckCircle } from 'lucide-svelte';
  import Button from './button.svelte';

  let isInstalling = $state(false);

  async function handleDownload() {
    isInstalling = true;
    try {
      await updater.downloadAndInstall();
    } catch (err) {
      console.error('Failed to download update:', err);
    }
    isInstalling = false;
  }

  async function handleRestart() {
    await updater.restartApp();
  }

  function handleDismiss() {
    updater.dismiss();
  }
</script>

{#if updater.isUpdateAvailable}
  <div class="fixed bottom-4 right-4 z-50 max-w-sm animate-in slide-in-from-bottom-4 duration-300">
    <div class="rounded-lg border bg-background shadow-lg p-4">
      <div class="flex items-start gap-3">
        {#if updater.status === 'ready'}
          <CheckCircle class="h-5 w-5 text-green-500 mt-0.5 shrink-0" />
        {:else}
          <Download class="h-5 w-5 text-primary mt-0.5 shrink-0" />
        {/if}

        <div class="flex-1 min-w-0">
          <h4 class="text-sm font-medium">
            {#if updater.status === 'ready'}
              Update Ready
            {:else}
              Update Available
            {/if}
          </h4>
          <p class="text-sm text-muted-foreground mt-0.5">
            {#if updater.status === 'ready'}
              Version {updater.version} is ready to install. Restart to apply.
            {:else if updater.status === 'downloading'}
              Downloading update...
            {:else}
              Version {updater.version} is available.
            {/if}
          </p>

          <div class="flex items-center gap-2 mt-3">
            {#if updater.status === 'ready'}
              <Button size="sm" onclick={handleRestart}>
                <RefreshCw class="h-4 w-4 mr-1.5" />
                Restart Now
              </Button>
            {:else if updater.status === 'downloading'}
              <Button size="sm" disabled>
                <Loader2 class="h-4 w-4 mr-1.5 animate-spin" />
                Downloading...
              </Button>
            {:else}
              <Button size="sm" onclick={handleDownload} disabled={isInstalling}>
                <Download class="h-4 w-4 mr-1.5" />
                Download
              </Button>
            {/if}

            <Button size="sm" variant="ghost" onclick={handleDismiss}>
              Later
            </Button>
          </div>
        </div>

        <button
          onclick={handleDismiss}
          class="text-muted-foreground hover:text-foreground transition-colors shrink-0"
        >
          <X class="h-4 w-4" />
          <span class="sr-only">Dismiss</span>
        </button>
      </div>
    </div>
  </div>
{/if}
