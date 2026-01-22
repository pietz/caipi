<script lang="ts">
  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 32, class: className = '' }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 32 32"
  fill="none"
  class={className}
>
  <rect x="2" y="5" width="28" height="22" rx="3" fill="#1a1a1a" stroke="#333" stroke-width="1" />
  <rect x="4" y="7" width="24" height="2" rx="1" fill="#333" />
  <circle cx="6" cy="8" r="1" fill="#ff5f57" />
  <circle cx="9" cy="8" r="1" fill="#febc2e" />
  <circle cx="12" cy="8" r="1" fill="#28c840" />
  <path d="M8 15l4 3-4 3" stroke="#3b82f6" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
  <path d="M14 21h6" stroke="#666" stroke-width="1.5" stroke-linecap="round" />
</svg>
