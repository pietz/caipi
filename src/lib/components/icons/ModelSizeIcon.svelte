<script lang="ts">
	import type { Backend } from '$lib/config/backends';
	import { Cpu } from 'lucide-svelte';

	interface Props {
		size: 'large' | 'medium' | 'small';
		backend?: Backend;
		class?: string;
	}
	let { size: _size, backend: _backend = 'claude', class: className = '' }: Props = $props();
</script>

<Cpu size={14} class={className} />
