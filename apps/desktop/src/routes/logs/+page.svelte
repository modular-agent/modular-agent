<script lang="ts">
  import { appLogDir } from "@tauri-apps/api/path";
  import { openPath } from "@tauri-apps/plugin-opener";

  import { onMount, tick } from "svelte";

  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { logStore, LogLevel, type LogEntry } from "$lib/log-store.svelte";
  import { titlebarState } from "$lib/titlebar-state.svelte";

  let selectedLevel = $state("all");
  let searchText = $state("");
  let autoScroll = $state(true);
  let scrollContainer: HTMLDivElement | undefined = $state();

  const levelOptions = [
    { value: "all", label: "All Levels" },
    { value: String(LogLevel.Trace), label: "Trace" },
    { value: String(LogLevel.Debug), label: "Debug" },
    { value: String(LogLevel.Info), label: "Info" },
    { value: String(LogLevel.Warn), label: "Warn" },
    { value: String(LogLevel.Error), label: "Error" },
  ];

  let filteredEntries: LogEntry[] = $derived.by(() => {
    let entries = logStore.entries;
    if (selectedLevel !== "all") {
      const level = Number(selectedLevel);
      entries = entries.filter((e) => e.level >= level);
    }
    if (searchText) {
      const lower = searchText.toLowerCase();
      entries = entries.filter((e) => e.message.toLowerCase().includes(lower));
    }
    return entries;
  });

  let prevLength = 0;
  $effect(() => {
    const len = filteredEntries.length;
    if (autoScroll && len !== prevLength && scrollContainer) {
      prevLength = len;
      tick().then(() => {
        scrollContainer?.scrollTo({ top: scrollContainer.scrollHeight });
      });
    }
  });

  onMount(() => {
    titlebarState.reset();
    titlebarState.title = "Logs";
  });

  function getLevelLabel(level: LogLevel): string {
    switch (level) {
      case LogLevel.Trace:
        return "TRACE";
      case LogLevel.Debug:
        return "DEBUG";
      case LogLevel.Info:
        return "INFO";
      case LogLevel.Warn:
        return "WARN";
      case LogLevel.Error:
        return "ERROR";
      default:
        return "?";
    }
  }

  function getLevelColor(level: LogLevel): string {
    switch (level) {
      case LogLevel.Trace:
      case LogLevel.Debug:
        return "text-muted-foreground";
      case LogLevel.Info:
        return "text-foreground";
      case LogLevel.Warn:
        return "text-yellow-500";
      case LogLevel.Error:
        return "text-destructive";
      default:
        return "text-foreground";
    }
  }

  function formatTime(date: Date): string {
    return date.toLocaleTimeString("en-US", {
      hour12: false,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      fractionalSecondDigits: 3,
    } as Intl.DateTimeFormatOptions);
  }

  function getLevelDisplay(): string {
    const opt = levelOptions.find((o) => o.value === selectedLevel);
    return opt?.label ?? "All Levels";
  }

  async function openLogFolder() {
    try {
      const logDir = await appLogDir();
      await openPath(logDir);
    } catch (e) {
      console.error("Failed to open log folder:", e);
    }
  }
</script>

<div class="flex flex-col w-full h-full">
  <div class="flex-none px-4 pt-4 pb-2">
    <header class="flex items-center justify-between h-14">
      <div class="text-2xl font-semibold">Logs</div>
      <div class="flex items-center gap-2">
        <Input
          type="text"
          placeholder="Search..."
          class="w-48 h-8"
          bind:value={searchText}
        />

        <Select.Root type="single" value={selectedLevel} onValueChange={(v) => (selectedLevel = v)}>
          <Select.Trigger class="w-[130px]" size="sm">
            {getLevelDisplay()}
          </Select.Trigger>
          <Select.Content>
            {#each levelOptions as option (option.value)}
              <Select.Item value={option.value}>{option.label}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>

        <Button
          variant={autoScroll ? "default" : "outline"}
          size="sm"
          onclick={() => (autoScroll = !autoScroll)}
        >
          Auto-scroll
        </Button>

        <Button variant="outline" size="sm" onclick={openLogFolder}>
          Open Log Folder
        </Button>

        <Button variant="outline" size="sm" onclick={() => logStore.clear()}>
          Clear
        </Button>
      </div>
    </header>
  </div>

  <div
    class="flex-1 overflow-auto px-4 pb-4"
    bind:this={scrollContainer}
  >
    <div class="font-mono text-xs leading-5">
      {#each filteredEntries as entry (entry.id)}
        <div class="flex gap-2 py-0.5 hover:bg-muted/50 rounded px-1">
          <span class="text-muted-foreground whitespace-nowrap flex-shrink-0">
            {formatTime(entry.timestamp)}
          </span>
          <span
            class="font-semibold whitespace-nowrap flex-shrink-0 w-12 {getLevelColor(entry.level)}"
          >
            {getLevelLabel(entry.level)}
          </span>
          <span class="break-all {getLevelColor(entry.level)}">
            {entry.message}
          </span>
        </div>
      {/each}
      {#if filteredEntries.length === 0}
        <div class="text-muted-foreground text-center py-8">No log entries</div>
      {/if}
    </div>
  </div>
</div>
