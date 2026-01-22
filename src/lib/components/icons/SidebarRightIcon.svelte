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
  <rect x="2" y="3" width="12" height="10" rx="1.5" />
  <path d="M10 3v10" />
</svg>
