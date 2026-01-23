<script lang="ts">
  import TodoList from './TodoList.svelte';
  import SkillsList from './SkillsList.svelte';
</script>

<div class="flex flex-col h-full" style="background-color: var(--sidebar);">
  <div class="flex-1 min-h-0 overflow-y-auto" style="border-bottom: 1px solid var(--border);">
    <TodoList />
  </div>
  <div class="flex-1 min-h-0 overflow-y-auto">
    <SkillsList />
  </div>
</div>
