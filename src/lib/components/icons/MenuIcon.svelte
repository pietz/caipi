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
  <path d="M2 4h12" stroke-linecap="round" />
  <path d="M2 8h12" stroke-linecap="round" />
  <path d="M2 12h12" stroke-linecap="round" />
</svg>
