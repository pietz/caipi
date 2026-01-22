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
  <path d="M2.5 2.5l11 5.5-11 5.5v-4l7-1.5-7-1.5v-4z" />
</svg>
