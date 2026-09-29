<script lang="ts">
import { Frame, Hero, Note } from "@perishlab/design";
import { onMount } from "svelte";
import { type Health, probe } from "../lib/hooks/health";

let health = $state<Health | undefined>(undefined);

onMount(() =>
	probe((value) => {
		health = value;
	}),
);
</script>

<Frame>
	<Hero title="stim" line="a message product downstream of santi" mark="◆" />
	{#if health === undefined}
		<Note text="The api has not answered yet." mood="warn" />
	{:else}
		<Note text={`api: ${health.status}`} />
	{/if}
</Frame>
