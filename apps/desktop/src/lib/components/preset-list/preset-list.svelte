<script lang="ts" module>
  import {
    type ColumnDef,
    type ColumnFiltersState,
    type VisibilityState,
    getCoreRowModel,
    getFilteredRowModel,
  } from "@tanstack/table-core";

  import PresetStatus from "$lib/components/preset-status.svelte";
  import {
    createSvelteTable,
    FlexRender,
    renderComponent,
  } from "$lib/components/ui/data-table/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import type { PresetInfoExt } from "$lib/types";

  import PresetListActions from "./preset-list-actions.svelte";
  import PresetListName from "./preset-list-name.svelte";

  const STATUS_COL_WIDTH = "w-[220px]";
  const ACTIONS_COL_WIDTH = "w-[140px]";

  type Props = {
    presets: PresetInfoExt[];
  };
</script>

<script lang="ts">
  let { presets }: Props = $props();

  let columnFilters = $state<ColumnFiltersState>([]);
  let columnVisibility = $state<VisibilityState>({});

  const columns: ColumnDef<PresetInfoExt>[] = [
    {
      id: "name",
      header: "Name",
      accessorFn: (row) => row.name,
      filterFn: "includesString",
      cell: ({ row }) => {
        return renderComponent(PresetListName, {
          id: row.original.id,
          name: row.original.name,
        });
      },
      meta: {
        headerClass: "w-full px-2",
        cellClass: "w-full px-2",
      },
    },
    {
      id: "status",
      header: "Status",
      cell: ({ row }) => {
        return renderComponent(PresetStatus, {
          running: row.original.running,
          run_on_start: row.original.run_on_start,
          class: "w-full justify-end",
        });
      },
      meta: {
        headerClass: `${STATUS_COL_WIDTH} pl-4`,
        cellClass: `${STATUS_COL_WIDTH} pl-4`,
      },
    },
    {
      id: "actions",
      header: "Actions",
      cell: ({ row }) => {
        return renderComponent(PresetListActions, {
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
    },
  ];

  const table = createSvelteTable({
    get data() {
      return presets;
    },
    columns,
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    onColumnFiltersChange: (updater) => {
      if (typeof updater === "function") {
        columnFilters = updater(columnFilters);
      } else {
        columnFilters = updater;
      }
    },
    onColumnVisibilityChange: (updater) => {
      if (typeof updater === "function") {
        columnVisibility = updater(columnVisibility);
      } else {
        columnVisibility = updater;
      }
    },
    state: {
      get columnFilters() {
        return columnFilters;
      },
      get columnVisibility() {
        return columnVisibility;
      },
    },
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
                  <FlexRender
                    content={header.column.columnDef.header}
                    context={header.getContext()}
                  />
                {/if}
              </Table.Head>
            {/each}
          </Table.Row>
        {/each}
      </Table.Header>
      <Table.Body>
        {#each table.getRowModel().rows as row (row.id)}
          <Table.Row data-state={row.getIsSelected() && "selected"}>
            {#each row.getVisibleCells() as cell (cell.id)}
              <Table.Cell class={cell.column.columnDef.meta?.cellClass}>
                <FlexRender content={cell.column.columnDef.cell} context={cell.getContext()} />
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
