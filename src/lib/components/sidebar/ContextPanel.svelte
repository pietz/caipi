<script lang="ts">
  import TodoList from './TodoList.svelte';
  import SkillsList from './SkillsList.svelte';
</script>

<div class="h-full flex flex-col">
  <!-- Todos - grows to fill available space -->
  <div class="flex-1 flex flex-col min-h-0 border-b border-border">
    <TodoList />
  </div>

  <!-- Skills - grows to fill available space -->
  <div class="flex-1 flex flex-col min-h-0">
    <SkillsList />
  </div>
</div>
