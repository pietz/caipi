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
  viewBox="0 0 14 14"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  class={className}
>
  <path d="M3 7.5l2.5 2.5 5.5-6" stroke-linecap="round" stroke-linejoin="round" />
</svg>
