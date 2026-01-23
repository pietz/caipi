<script lang="ts">
  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 16, class: className = '' }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.5"
  class={className}
>
  <path d="M4 4l8 8" stroke-linecap="round" stroke-linejoin="round" />
  <path d="M12 4l-8 8" stroke-linecap="round" stroke-linejoin="round" />
</svg>
