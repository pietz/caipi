<script lang="ts">
  import TodoList from './TodoList.svelte';
  import SkillsList from './SkillsList.svelte';
  import { LicenseInfo } from '$lib/components/license';
  import { app } from '$lib/stores/app.svelte';

  const hasLicense = $derived(app.license?.valid ?? false);
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

  <!-- License Info - fixed at bottom -->
  {#if hasLicense}
    <LicenseInfo />
  {/if}
</div>
