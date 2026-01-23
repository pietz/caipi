<script lang="ts">
  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 14, class: className = '' }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  class={className}
>
  <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline>
</svg>
