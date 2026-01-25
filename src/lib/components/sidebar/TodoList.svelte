<script lang="ts">
  import { CheckCircle2, Circle, Loader2 } from 'lucide-svelte';
  import { chat } from '$lib/stores/chat.svelte';
</script>

<div class="p-3 pb-0">
  <div class="text-xs uppercase tracking-widest font-semibold mb-3 text-muted-foreground/50">
    Todos
  </div>
</div>
<div class="flex-1 overflow-y-auto px-3 pb-3">
  {#if chat.todos.length === 0}
    <div class="text-xs text-muted-foreground">No active todos</div>
  {:else}
    <div class="space-y-1">
      {#each chat.todos as todo (todo.id)}
        <div class="flex items-start gap-2 p-2 rounded-lg transition-colors {todo.active ? 'bg-orange-500/10 border border-orange-500/20' : ''}">
          <span class="mt-0.5 {todo.done ? 'text-green-500' : todo.active ? 'text-orange-500' : 'text-muted-foreground/50'}">
            {#if todo.done}
              <CheckCircle2 size={14} />
            {:else if todo.active}
              <Loader2 size={14} class="animate-spin" />
            {:else}
              <Circle size={14} />
            {/if}
          </span>
          <span class="text-xs {todo.done ? 'text-muted-foreground line-through' : 'text-foreground/80'}">
            {todo.text}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</div>
