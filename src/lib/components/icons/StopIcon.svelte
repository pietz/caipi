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
  fill="currentColor"
  class={className}
>
  <rect x="3" y="3" width="10" height="10" rx="1" />
</svg>
