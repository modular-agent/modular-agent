# Example workspace

A workspace that exercises `ma serve` and `ma run` with standard modules only:
no external services, no secrets. `apps/cli/tests/workspace_test.rs` runs it
as an integration test.

```text
settings.json   # auto-starts all five patches
patches/        # one file per patch; a "/" in a patch name is a subfolder
data/           # created by the journal patch
```

| Patch | Wiring | Shows |
| --- | --- | --- |
| `heartbeat` | Interval Timer (5s) → Counter → Log (info) | auto-start, Log node records in the log |
| `echo` | `input` → Template String → `output`, plus Log (debug) | stdin/stdout with `ma run`, debug-level filtering |
| `parse` | `parse` → From JSON → `parsed`; `err` → Log (error) | errors reaching the log |
| `journal` | `journal` → Append JSONL File (`data/journal.jsonl`) | relative paths resolving against the workspace |
| `tick` | Schedule Timer (`*/10 * * * * *`) → Log (info) | cron schedules, evaluated in UTC |

## ma run

`--workspace` makes the patch argument a name in the workspace and resolves
relative paths in patches against it.

```sh
echo hi | ma run --workspace examples/workspace --drain 1s echo
# echo: hi

echo '"{\"a\":[1,true]}"' | ma run --workspace examples/workspace --json --drain 1s -i parse -o parsed parse
# {"a":[1,true]}

echo nope | ma run --workspace examples/workspace --drain 1s -i parse -o parsed parse
# stderr: ... ERROR modular_agent::patch] bad input: ...

RUST_LOG=modular_agent::=debug ma run --workspace examples/workspace echo
# also shows the debug-level Log node and the values on each channel
```

`--drain` keeps writing outputs for a while after stdin ends; without it, the
shutdown that EOF starts can cut off the last values.

## ma serve

```sh
ma --log-format json serve --workspace examples/workspace --listen 127.0.0.1:8080
curl http://127.0.0.1:8080/healthz   # 200 once the patches are running
```

Logs go to stderr, one JSON object per line. `/mcp` is the MCP server (for
example, for Claude Code); it needs no token on a loopback address.

## In a container

Build the image from the repository root and mount a copy of this workspace
(the journal patch writes into it):

```sh
docker build -f apps/cli/Dockerfile -t modular-agent .
cp -r examples/workspace /tmp/ma-workspace
docker run --name ma -e MA_TOKEN=change-me -p 8080:8080 -v /tmp/ma-workspace:/workspace modular-agent
```

- Listening beyond loopback requires `MA_TOKEN`; `/mcp` then needs
  `Authorization: Bearer <MA_TOKEN>`. `/healthz` stays open for health checks.
- `docker stop ma` sends SIGTERM: the patches stop (`patch stopped` in the log)
  and the container exits with status 0.
- `data/journal.jsonl` stays in the mounted directory across container
  restarts.
