<script lang="ts">
  import { ActivityIcon } from '$lib/components/icons';
  import { chat } from '$lib/stores/chat.svelte';
</script>

<div class="p-3">
  <div class="text-xs font-medium text-muted-foreground uppercase tracking-[0.5px] mb-2.5">
    Skills
  </div>

  {#if chat.activeSkills.length === 0}
    <div class="text-sm text-dim">No active skills</div>
  {:else}
    <div class="flex flex-col gap-2">
      {#each chat.activeSkills as skill}
        <div class="skill-item">
          <ActivityIcon size={14} class="text-purple-400" />
          <span>{skill}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .skill-item {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-secondary);
  }
</style>
