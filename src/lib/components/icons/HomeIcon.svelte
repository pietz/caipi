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
  <path d="M2.5 6.5L8 2l5.5 4.5" stroke-linecap="round" stroke-linejoin="round" />
  <path d="M4 5.5V13h3v-4h2v4h3V5.5" stroke-linecap="round" stroke-linejoin="round" />
</svg>
