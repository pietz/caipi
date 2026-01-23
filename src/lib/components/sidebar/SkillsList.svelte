<script lang="ts">
  import { ActivityIcon } from '$lib/components/icons';
  import { chatStore } from '$lib/stores';

  let activeSkills = $state<string[]>([]);

  chatStore.subscribe((state) => {
    activeSkills = state.activeSkills;
  });
</script>

<div class="p-3">
  <div class="text-xs font-medium text-muted uppercase tracking-[0.5px] mb-2.5">
    Active Skills
  </div>

  {#if activeSkills.length === 0}
    <div class="text-sm text-dim">No active skills</div>
  {:else}
    <div class="flex flex-col gap-2">
      {#each activeSkills as skill}
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
