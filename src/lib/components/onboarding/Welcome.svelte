<script lang="ts">
  import { CaipiIcon, CheckIcon } from '$lib/components/icons';

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
    <h1 class="text-lg font-semibold mt-4 text-primary">
      Caipi
    </h1>
    <p class="text-xs text-muted mt-1">
      A friendly UI for Claude Code
    </p>
  </div>

  <!-- Install Instructions Card -->
  <div
    class="w-[340px] rounded-lg p-5"
    style="background: var(--card); border: 1px solid var(--border);"
  >
    <div class="text-sm font-medium text-primary mb-3">
      Claude Code CLI Required
    </div>

    <p class="text-xs text-secondary mb-4">
      Caipi requires the Claude Code CLI. Run this in your terminal:
    </p>

    <div class="flex items-center gap-2">
      <code
        class="flex-1 text-xs px-3 py-2 rounded overflow-x-auto"
        style="background: var(--bg); border: 1px solid var(--border); color: var(--text-secondary);"
      >
        {installCommand}
      </code>
      <button
        type="button"
        onclick={copyToClipboard}
        class="shrink-0 p-2 rounded transition-all duration-150"
        style="
          background: var(--hover);
          color: {copied ? 'var(--accent)' : 'var(--text-muted)'};
        "
        title="Copy to clipboard"
      >
        {#if copied}
          <CheckIcon size={14} />
        {:else}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
          </svg>
        {/if}
      </button>
    </div>

    <p class="text-xs text-muted mt-4">
      After installing, restart Caipi to continue.
    </p>
  </div>
</div>
