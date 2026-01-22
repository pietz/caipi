<script lang="ts">
  interface Props {
    size?: number;
    expanded?: boolean;
    class?: string;
  }
  let { size = 12, expanded = false, class: className = '' }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 12 12"
  fill="currentColor"
  class={className}
  style="transform: rotate({expanded ? 90 : 0}deg); transition: transform 0.15s ease;"
>
  <path d="M4.5 2.5l4 3.5-4 3.5V2.5z" />
</svg>
