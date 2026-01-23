<script lang="ts">
  import { chatStore, type TodoItem } from '$lib/stores';

  const todos = $derived($chatStore.todos);
</script>

<div class="p-3">
  <div class="text-xs font-medium text-muted uppercase tracking-[0.5px] mb-2.5">
    To-Do List
  </div>

  {#if todos.length === 0}
    <div class="text-sm text-dim">No active to-dos</div>
  {:else}
    <div class="todo-list">
      {#each todos as todo (todo.id)}
        <div class="todo-item" class:done={todo.done}>
          <!-- Checkbox indicator -->
          {#if todo.done}
            <div class="checkbox completed">
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="20 6 9 17 4 12"></polyline>
              </svg>
            </div>
          {:else if todo.active}
            <div class="checkbox active">
              <div class="dot"></div>
            </div>
          {:else}
            <div class="checkbox pending"></div>
          {/if}

          <span class="todo-text">{todo.text}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .todo-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .todo-item {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .todo-item.done {
    color: var(--text-dim);
  }

  .todo-item.done .todo-text {
    text-decoration: line-through;
  }

  .checkbox {
    width: 14px;
    height: 14px;
    min-width: 14px;
    min-height: 14px;
    border-radius: 3px;
    flex-shrink: 0;
    margin-top: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
  }

  .checkbox.completed {
    background-color: #22c55e;
  }

  .checkbox.active {
    border: 1.5px solid #3b82f6;
    background-color: transparent;
  }

  .checkbox.pending {
    border: 1.5px solid #737373;
    background-color: transparent;
  }

  .checkbox.active .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background-color: #3b82f6;
  }
</style>
