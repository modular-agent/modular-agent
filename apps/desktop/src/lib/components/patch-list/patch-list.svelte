<script lang="ts" module>
  import {
    columnFilteringFeature,
    columnVisibilityFeature,
    createColumnHelper,
    createFilteredRowModel,
    createTable,
    filterFn_includesString,
    FlexRender,
    renderComponent,
    tableFeatures,
  } from "@tanstack/svelte-table";

  import PatchStatus from "$lib/components/patch-status.svelte";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import type { PatchInfoExt } from "$lib/types";

  import PatchListActions from "./patch-list-actions.svelte";
  import PatchListName from "./patch-list-name.svelte";

  const STATUS_COL_WIDTH = "w-[220px]";
  const ACTIONS_COL_WIDTH = "w-[140px]";

  // v9 tree-shakes filter functions: a filterFn named by string silently
  // no-ops unless it is registered here.
  const features = tableFeatures({
    columnFilteringFeature,
    columnVisibilityFeature,
    filteredRowModel: createFilteredRowModel(),
    filterFns: { includesString: filterFn_includesString },
  });

  const columnHelper = createColumnHelper<typeof features, PatchInfoExt>();

  type Props = {
    patches: PatchInfoExt[];
  };
</script>

<script lang="ts">
  let { patches }: Props = $props();

  const columns = columnHelper.columns([
    columnHelper.accessor((row) => row.name, {
      id: "name",
      header: "Name",
      filterFn: "includesString",
      cell: ({ row }) => {
        return renderComponent(PatchListName, {
          id: row.original.id,
          name: row.original.name,
        });
      },
      meta: {
        headerClass: "w-full px-2",
        cellClass: "w-full px-2",
      },
    }),
    columnHelper.display({
      id: "status",
      header: "Status",
      cell: ({ row }) => {
        return renderComponent(PatchStatus, {
          running: row.original.running,
          run_on_start: row.original.run_on_start,
          class: "w-full justify-end",
        });
      },
      meta: {
        headerClass: `${STATUS_COL_WIDTH} pl-4`,
        cellClass: `${STATUS_COL_WIDTH} pl-4`,
      },
    }),
    columnHelper.display({
      id: "actions",
      header: "Actions",
      cell: ({ row }) => {
        return renderComponent(PatchListActions, {
          id: row.original.id,
          name: row.original.name,
          running: row.original.running,
          run_on_start: row.original.run_on_start,
        });
      },
      meta: {
        headerClass: `${ACTIONS_COL_WIDTH} pl-4`,
        cellClass: `${ACTIONS_COL_WIDTH} pl-4`,
      },
    }),
  ]);

  const table = createTable({
    features,
    get data() {
      return patches;
    },
    columns,
  });
</script>

<div class="text-primary w-full">
  <div class="flex items-center justify-between pb-4">
    <div class="w-64">
      <Input
        value={(table.getColumn("name")?.getFilterValue() as string) ?? ""}
        onchange={(e) => {
          table.getColumn("name")?.setFilterValue(e.currentTarget.value);
        }}
        oninput={(e) => {
          table.getColumn("name")?.setFilterValue(e.currentTarget.value);
        }}
      />
    </div>
  </div>
  <div class="">
    <Table.Root>
      <Table.Header class="bg-muted">
        {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
          <Table.Row>
            {#each headerGroup.headers as header (header.id)}
              <Table.Head
                colspan={header.colSpan}
                class={header.column.columnDef.meta?.headerClass}
              >
                {#if !header.isPlaceholder}
                  <FlexRender {header} />
                {/if}
              </Table.Head>
            {/each}
          </Table.Row>
        {/each}
      </Table.Header>
      <Table.Body>
        {#each table.getRowModel().rows as row (row.id)}
          <Table.Row>
            {#each row.getVisibleCells() as cell (cell.id)}
              <Table.Cell class={cell.column.columnDef.meta?.cellClass}>
                <FlexRender {cell} />
              </Table.Cell>
            {/each}
          </Table.Row>
        {:else}
          <Table.Row>
            <Table.Cell colspan={columns.length} class="h-24 text-center">No results.</Table.Cell>
          </Table.Row>
        {/each}
      </Table.Body>
    </Table.Root>
  </div>
</div>
