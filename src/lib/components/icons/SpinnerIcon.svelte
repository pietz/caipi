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
  class="animate-spin {className}"
>
  <circle cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.25" />
  <path d="M7 1.5A5.5 5.5 0 0112.5 7" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
</svg>
