<script lang="ts">
	interface Props {
		size: 'large' | 'medium' | 'small';
		class?: string;
	}
	let { size, class: className = '' }: Props = $props();

	const rings = $derived(size === 'large' ? 3 : size === 'medium' ? 2 : 1);
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	width="14"
	height="14"
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="2"
	stroke-linecap="round"
	stroke-linejoin="round"
	class={className}
>
	{#if rings >= 3}
		<circle cx="12" cy="12" r="10" />
	{/if}
	{#if rings >= 2}
		<circle cx="12" cy="12" r="6" />
	{/if}
	<circle cx="12" cy="12" r="2" />
</svg>
