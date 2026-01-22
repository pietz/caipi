<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ShieldAlert, ChevronDown, ChevronUp } from 'lucide-svelte';
  import { Button, Dialog } from '$lib/components/ui';
  import type { PermissionRequest } from '$lib/stores';

  interface Props {
    request: PermissionRequest;
    onResponse: (allowed: boolean, remember: boolean) => void;
  }

  let { request, onResponse }: Props = $props();
  let showDetails = $state(false);
  let timeRemaining = $state(60);
  let timer: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    // 60 second timeout
    timer = setInterval(() => {
      timeRemaining--;
      if (timeRemaining <= 0) {
        onResponse(false, false);
      }
    }, 1000);
  });

  onDestroy(() => {
    if (timer) clearInterval(timer);
  });

  function handleDeny() {
    onResponse(false, false);
  }

  function handleAllowOnce() {
    onResponse(true, false);
  }

  function handleAllowAlways() {
    onResponse(true, true);
  }

  // Translate technical descriptions to user-friendly language
  function translateDescription(tool: string, description: string): string {
    // The description from the backend should already be translated
    // but we can add additional context here if needed
    return description;
  }

  const friendlyDescription = $derived(translateDescription(request.tool, request.description));
</script>

<Dialog open={true} title="Permission Required">
  <div class="space-y-4">
    <!-- Warning Icon -->
    <div class="flex items-center gap-3">
      <div class="flex-shrink-0 w-10 h-10 rounded-full bg-yellow-500/10 flex items-center justify-center">
        <ShieldAlert class="w-5 h-5 text-yellow-500" />
      </div>
      <div>
        <h3 class="font-medium">Claude wants to perform an action</h3>
        <p class="text-sm text-muted-foreground">
          This requires your permission
        </p>
      </div>
    </div>

    <!-- Description -->
    <div class="p-4 bg-muted rounded-lg">
      <p class="text-sm font-medium">{friendlyDescription}</p>
    </div>

    <!-- Details Toggle -->
    <button
      class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-colors"
      onclick={() => (showDetails = !showDetails)}
    >
      {#if showDetails}
        <ChevronUp class="w-4 h-4" />
        Hide technical details
      {:else}
        <ChevronDown class="w-4 h-4" />
        What does this mean?
      {/if}
    </button>

    {#if showDetails}
      <div class="p-3 bg-muted/50 rounded text-xs font-mono text-muted-foreground">
        <div><strong>Tool:</strong> {request.tool}</div>
        <div class="mt-1 break-all">{request.description}</div>
      </div>
    {/if}

    <!-- Timer -->
    <div class="text-center text-sm text-muted-foreground">
      Auto-denying in {timeRemaining} seconds
    </div>
  </div>

  {#snippet footer()}
    <Button variant="outline" onclick={handleDeny}>
      Deny
    </Button>
    <Button variant="secondary" onclick={handleAllowOnce}>
      Allow Once
    </Button>
    <Button onclick={handleAllowAlways}>
      Always Allow
    </Button>
  {/snippet}
</Dialog>
