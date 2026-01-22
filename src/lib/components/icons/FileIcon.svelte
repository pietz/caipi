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
  <path d="M4 2.5A1.5 1.5 0 015.5 1h3.379a1.5 1.5 0 011.06.44l2.122 2.12a1.5 1.5 0 01.439 1.061V12.5A1.5 1.5 0 0111 14H5.5A1.5 1.5 0 014 12.5v-10z" />
  <path d="M9 1v3.5a.5.5 0 00.5.5H13" />
</svg>
