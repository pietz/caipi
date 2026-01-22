<script lang="ts">
  import caipiLogo from '$lib/assets/caipi-logo.png';

  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 48, class: className = '' }: Props = $props();
</script>

<img
  src={caipiLogo}
  alt="Caipi"
  width={size}
  height={size}
  class={className}
/>
