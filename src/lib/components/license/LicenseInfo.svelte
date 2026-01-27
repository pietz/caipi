<script lang="ts">
  import { api } from '$lib/api';
  import { Key, LogOut, Loader2 } from 'lucide-svelte';
  import { Button } from '$lib/components/ui';
  import { app } from '$lib/stores/app.svelte';

  let deactivating = $state(false);

  const licenseKey = $derived(app.license?.licenseKey ?? 'Unknown');
  const email = $derived(app.license?.email ?? null);
  const activatedAt = $derived(
    app.license?.activatedAt
      ? new Date(app.license.activatedAt * 1000).toLocaleDateString()
      : null
  );

  async function deactivateLicense() {
    if (!confirm('Are you sure you want to deactivate your license? You will need to re-enter your license key.')) {
      return;
    }

    deactivating = true;
    try {
      await api.clearLicense();
      app.setLicense(null);
      app.setScreen('license');
    } catch (e) {
      console.error('Failed to deactivate license:', e);
    } finally {
      deactivating = false;
    }
  }
</script>

<div class="p-3 border-t border-border">
  <div class="flex items-center gap-2 mb-3">
    <Key size={14} class="text-muted-foreground" />
    <span class="text-xs font-medium text-foreground">License</span>
  </div>

  <div class="space-y-2 text-xs">
    <div class="flex justify-between">
      <span class="text-muted-foreground">Status</span>
      <span class="text-green-500">Active</span>
    </div>

    {#if email}
      <div class="flex justify-between">
        <span class="text-muted-foreground">Email</span>
        <span class="text-foreground truncate max-w-[120px]" title={email}>{email}</span>
      </div>
    {/if}

    {#if activatedAt}
      <div class="flex justify-between">
        <span class="text-muted-foreground">Activated</span>
        <span class="text-foreground">{activatedAt}</span>
      </div>
    {/if}

    <div class="flex justify-between items-center">
      <span class="text-muted-foreground">Key</span>
      <code class="text-foreground text-[10px] bg-muted px-1.5 py-0.5 rounded">{licenseKey}</code>
    </div>
  </div>

  <Button
    variant="ghost"
    size="sm"
    class="w-full mt-3 h-7 text-xs text-muted-foreground hover:text-red-500"
    onclick={deactivateLicense}
    disabled={deactivating}
  >
    {#if deactivating}
      <Loader2 size={12} class="animate-spin mr-1.5" />
      Deactivating...
    {:else}
      <LogOut size={12} class="mr-1.5" />
      Deactivate License
    {/if}
  </Button>
</div>
