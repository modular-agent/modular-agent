<script lang="ts" module>
  import {
    type ColumnDef,
    type ColumnFiltersState,
    type VisibilityState,
    getCoreRowModel,
    getFilteredRowModel,
  } from "@tanstack/table-core";

  import { goto } from "$app/navigation";

  import FlowStatus from "$lib/components/flow-status.svelte";
  import NewStreamDialog from "$lib/components/new-stream-dialog.svelte";
  import { buttonVariants } from "$lib/components/ui/button/index.js";
  import {
    createSvelteTable,
    FlexRender,
    renderComponent,
  } from "$lib/components/ui/data-table/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { newStream } from "$lib/shared.svelte";
  import type { AgentStreamInfoExt } from "$lib/types";

  import StreamListActions from "./stream-list-actions.svelte";
  import StreamListName from "./stream-list-name.svelte";

  declare module "@tanstack/table-core" {
    interface ColumnMeta<TData, TValue> {
      headerClass?: string;
      cellClass?: string;
    }
  }

  const STATUS_COL_WIDTH = "w-[220px]";
  const ACTIONS_COL_WIDTH = "w-[140px]";

  type Props = {
    streams: AgentStreamInfoExt[];
  };
</script>

<script lang="ts">
  let { streams }: Props = $props();

  let columnFilters = $state<ColumnFiltersState>([]);
  let columnVisibility = $state<VisibilityState>({});

  const columns: ColumnDef<AgentStreamInfoExt>[] = [
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
      meta: {
        headerClass: "w-full px-2",
        cellClass: "w-full px-2",
      },
    },
    {
      id: "status",
      header: "Status",
      cell: ({ row }) => {
        return renderComponent(FlowStatus, {
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
        return renderComponent(StreamListActions, {
          id: row.original.id,
          name: row.original.name,
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

  async function onNewStream(name: string) {
    const new_id = await newStream(name);
    if (new_id) {
      goto(`/stream_editor/${new_id}`, { invalidateAll: true });
    }
  }
</script>

<div class="text-primary p-4 w-full">
  <div class="text-lg font-semibold">Streams</div>
  <div class="flex items-center justify-between py-4">
    <div class="py-2 w-64">
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
    <NewStreamDialog {onNewStream}>
      {#snippet trigger()}
        <Dialog.Trigger class={buttonVariants({ variant: "outline" })}>+ New</Dialog.Trigger>
      {/snippet}
    </NewStreamDialog>
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
