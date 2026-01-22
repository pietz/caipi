<script lang="ts">
  import { CheckIcon } from '$lib/components/icons';
  import { chatStore, type TaskItem } from '$lib/stores';

  let tasks = $state<TaskItem[]>([]);

  chatStore.subscribe((state) => {
    tasks = state.tasks;
  });
</script>

<div class="p-3" style="border-bottom: 1px solid var(--border);">
  <div class="text-xs font-medium text-muted uppercase tracking-[0.5px] mb-2.5">
    Current Task
  </div>

  {#if tasks.length === 0}
    <div class="text-xs text-dim">No active tasks</div>
  {:else}
    <div class="flex flex-col gap-1.5">
      {#each tasks as task (task.id)}
        <div
          class="flex items-start gap-2 text-xs"
          style="color: {task.done ? 'var(--text-dim)' : task.active ? 'var(--text-primary)' : 'var(--text-muted)'};"
        >
          <span
            class="w-3.5 h-3.5 rounded flex items-center justify-center shrink-0 mt-[1px]"
            style="
              border: {task.done ? 'none' : '1px solid rgba(255,255,255,0.15)'};
              background-color: {task.done ? 'var(--accent-blue)' : task.active ? 'rgba(59, 130, 246, 0.2)' : 'transparent'};
            "
          >
            {#if task.done}
              <CheckIcon size={10} class="text-white" />
            {:else if task.active}
              <span
                class="w-1.5 h-1.5 rounded-full"
                style="background-color: var(--accent-blue);"
              ></span>
            {/if}
          </span>
          <span style="text-decoration: {task.done ? 'line-through' : 'none'};">
            {task.text}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</div>
