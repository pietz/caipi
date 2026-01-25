<script lang="ts">
  import { BookOpen } from 'lucide-svelte';
  import { chat } from '$lib/stores/chat.svelte';
</script>

<div class="p-3 pb-0">
  <div class="text-xs uppercase tracking-widest font-semibold mb-3 text-muted-foreground/50">
    Active Skills
  </div>
</div>
<div class="flex-1 overflow-y-auto px-3 pb-3">
  {#if chat.activeSkills.length === 0}
    <div class="text-xs text-muted-foreground">No active skills</div>
  {:else}
    <div class="space-y-2">
      {#each chat.activeSkills as skill}
        <div class="flex items-center gap-2 p-2 rounded-lg bg-muted/50">
          <BookOpen size={14} class="text-blue-500" />
          <div class="min-w-0">
            <div class="text-xs font-medium text-foreground/80">{skill}</div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
