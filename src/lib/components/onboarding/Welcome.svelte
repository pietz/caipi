<script lang="ts">
  import { Check, Copy } from 'lucide-svelte';
  import { CaipiIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';

  const installCommand = 'curl -fsSL https://claude.ai/install.sh | bash';
  let copied = $state(false);

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(installCommand);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }
</script>

<div class="flex flex-col items-center justify-center h-full gap-8 pt-12 px-10 pb-10" data-tauri-drag-region>
  <!-- Logo and Title -->
  <div class="flex flex-col items-center text-center">
    <CaipiIcon size={64} />
    <h1 class="text-lg font-semibold mt-4 text-foreground">
      Caipi
    </h1>
    <p class="text-xs text-muted-foreground mt-1">
      A friendly UI for Claude Code
    </p>
  </div>

  <!-- Install Instructions Card -->
  <div class="w-[340px] rounded-lg border border-border bg-card p-5">
    <div class="text-sm font-medium text-foreground mb-3">
      Claude Code CLI Required
    </div>

    <p class="text-xs text-muted-foreground mb-4">
      Caipi requires the Claude Code CLI. Run this in your terminal:
    </p>

    <div class="flex items-center gap-2">
      <code class="flex-1 text-xs px-3 py-2 rounded overflow-x-auto bg-muted border border-border text-muted-foreground">
        {installCommand}
      </code>
      <Button
        variant="outline"
        size="icon"
        class="shrink-0 h-8 w-8 {copied ? 'text-green-500' : ''}"
        onclick={copyToClipboard}
        title="Copy to clipboard"
      >
        {#if copied}
          <Check size={14} />
        {:else}
          <Copy size={14} />
        {/if}
      </Button>
    </div>

    <p class="text-xs text-muted-foreground mt-4">
      After installing, restart Caipi to continue.
    </p>
  </div>
</div>
