<script lang="ts">
  import {
    type ColumnDef,
    type ColumnFiltersState,
    type VisibilityState,
    getCoreRowModel,
    getFilteredRowModel,
  } from "@tanstack/table-core";
  import type { AgentStreamInfo } from "tauri-plugin-askit-api";

  import FlowStatus from "$lib/components/flow-status.svelte";
  import {
    createSvelteTable,
    FlexRender,
    renderComponent,
  } from "$lib/components/ui/data-table/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Table from "$lib/components/ui/table/index.js";

  import NewStreamDialog from "./new-stream-dialog.svelte";
  import StreamListActions from "./stream-list-actions.svelte";
  import StreamListName from "./stream-list-name.svelte";

  type Props = {
    streams: AgentStreamInfo[];
  };

  let { streams }: Props = $props();

  let columnFilters = $state<ColumnFiltersState>([]);
  let columnVisibility = $state<VisibilityState>({});

  const columns: ColumnDef<AgentStreamInfo>[] = [
    {
      id: "name",
      header: "Name",
      accessorFn: (row) => row.name,
      filterFn: "includesString",
      cell: ({ row }) => {
        return renderComponent(StreamListName, {
          id: row.original.id,
          name: row.original.name,
        });
      },
    },
    {
      id: "status",
      header: "Status",
      cell: ({ row }) => {
        return renderComponent(FlowStatus, {
          running: row.original.running,
          run_on_start: row.original.run_on_start,
        });
      },
    },
    {
      id: "actions",
      header: "Actions",
      cell: ({ row }) => {
        return renderComponent(StreamListActions, {
          id: row.original.id,
          name: row.original.name,
        });
      },
    },
  ];

  const table = createSvelteTable({
    get data() {
      return streams;
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

<div class="text-primary p-2 w-full">
  <h4>Streams</h4>
  <div class="flex items-center justify-between p-2">
    <div class="py-2">
      <Input
        value={(table.getColumn("name")?.getFilterValue() as string) ?? ""}
        onchange={(e) => {
          table.getColumn("name")?.setFilterValue(e.currentTarget.value);
        }}
        oninput={(e) => {
          table.getColumn("name")?.setFilterValue(e.currentTarget.value);
        }}
        class="max-w-sm"
      />
    </div>
    <NewStreamDialog />
  </div>
  <div class="">
    <Table.Root>
      <Table.Header class="bg-muted">
        {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
          <Table.Row>
            {#each headerGroup.headers as header (header.id)}
              <Table.Head colspan={header.colSpan}>
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
              <Table.Cell>
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
