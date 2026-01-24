<script lang="ts">
  import TodoList from './TodoList.svelte';
  import TaskList from './TaskList.svelte';
  import SkillsList from './SkillsList.svelte';
</script>

<div class="flex flex-col h-full" style="background-color: var(--sidebar);">
  <div class="flex-1 min-h-0 overflow-y-auto border-b border-border">
    <TodoList />
  </div>
  <div class="flex-1 min-h-0 overflow-y-auto border-b border-border">
    <TaskList />
  </div>
  <div class="flex-1 min-h-0 overflow-y-auto">
    <SkillsList />
  </div>
</div>
