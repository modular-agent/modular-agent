# CLAUDE.md

See root CLAUDE.md for common agent development patterns.

## Overview

Standard utility agents library with 50+ agents for data manipulation, file I/O, timing, and templating.

## Categories

| Category | Purpose | Key Agents |
| -------- | ------- | ---------- |
| Std/Array | Array operations | Map, Collect, ArrayFirst, ArrayLength, ZipToArray |
| Std/Data | Object operations | Get Value, Set Value, To JSON, From JSON, ZipToObject |
| Std/File | File I/O | Read/Write Text, JSON, JSONL, Glob, List Files, Watch Directory |
| Std/Input | UI inputs | String/Integer/Boolean/Number/Text/Object Input |
| Std/String | Text operations | Template String, String Join, String Length Split |
| Std/Time | Timing | Delay, Interval Timer, Schedule Timer, Throttle, On Start |
| Std/Filter | Conditional routing | If, Switch, Match |
| Std/Image | Image processing | Resize, Resample, Scale, IsBlank, IsChanged |
| Std/Sequence | Control flow | Sequence, Sync |
| Std/Display | Debugging | Display Value, Debug Value |
| Std/Utils | Utilities | Counter |
| Std/UI | UI elements | Note, Router |
| Std/YAML | YAML support | To YAML, From YAML |

## Features

- `image` (default) - Image processing agents (photon-rs)
- `yaml` (default) - YAML serialization (serde_yaml_ng)
- `watch` (default) - Directory watching (notify-debouncer-full)

## Key Patterns

### Map/Collect

Array iteration with context preservation:

```text
[Array] → Map → [Process Each] → Collect → [Continue]
```

Maintains map frames (index, length) through pipeline.

### Sync

Multi-input synchronization with two modes:

- **FIFO Mode**: Simple queues, first complete set outputs
- **Context Mode** (`use_ctx=true`): Groups by context key, handles interleaved inputs

```text
Sequence(n=2) → [Process1] → Sync → [Continue]
             → [Process2] ↗
```

### ZipToArray / ZipToObject

Combine multiple inputs into single output:

- Configurable number of inputs (`n` config)
- FIFO or context-aware mode (`use_ctx`)
- TTL caching to prevent memory leaks

### Template

Handlebars templating with built-in helpers:

- `{{to_json value}}` - JSON serialize
- `{{to_yaml value}}` - YAML serialize (with yaml feature)

No-escape mode enabled by default.

## File Agents

All file agents support dual inputs:

- Direct value (string, path)
- Doc object with `path` field override

JSONL agents support streaming append operations.

## Time Formats

- Duration: "10ms", "5s", "2m", "1h", "3d"
- Cron: Standard cron syntax for Schedule Timer
