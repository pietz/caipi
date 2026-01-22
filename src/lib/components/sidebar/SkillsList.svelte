<script lang="ts">
  import { SkillIcon } from '$lib/components/icons';
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
    <div class="text-xs text-dim">No active skills</div>
  {:else}
    <div class="flex flex-col gap-1.5">
      {#each activeSkills as skill}
        <div class="flex items-center gap-2 text-xs text-selected">
          <SkillIcon size={12} />
          <span>{skill}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
