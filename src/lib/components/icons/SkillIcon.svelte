<script lang="ts">
  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 12, class: className = '' }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 12 12"
  fill="currentColor"
  opacity="0.5"
  class={className}
>
  <circle cx="6" cy="6" r="2" />
</svg>
