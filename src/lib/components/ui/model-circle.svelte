<script lang="ts">
  interface Props {
    size: 'large' | 'medium' | 'small';
  }

  let { size }: Props = $props();

  const sizeClasses = {
    large: 'w-2.5 h-2.5',
    medium: 'w-2 h-2',
    small: 'w-1.5 h-1.5',
  };
</script>

<div class="{sizeClasses[size]} rounded-full bg-current opacity-70"></div>
