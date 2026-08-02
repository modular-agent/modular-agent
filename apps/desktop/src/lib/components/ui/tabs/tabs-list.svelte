<script lang="ts" module>
	export type TabsListVariant = "default" | "line";
</script>

<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		variant = "default",
		class: className,
		...restProps
	}: TabsPrimitive.ListProps & {
		variant?: TabsListVariant;
	} = $props();
</script>

<TabsPrimitive.List
	bind:ref
	data-slot="tabs-list"
	data-variant={variant}
	class={cn(
		"text-muted-foreground group/tabs-list inline-flex h-9 w-fit items-center justify-center",
		variant === "line" ? "gap-1 bg-transparent" : "bg-muted rounded-lg p-[3px]",
		className
	)}
	{...restProps}
/>
