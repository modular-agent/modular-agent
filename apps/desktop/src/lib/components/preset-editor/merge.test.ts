import type {
  AgentSpec,
  ConnectionSpec,
  PresetInfo,
  PresetSpec,
} from "tauri-plugin-modular-agent-api";
import { describe, expect, it } from "vitest";

import { presetToFlow } from "$lib/agent";
import type { PresetFlow } from "$lib/types";

import { connKey, reconcileFlow } from "./merge";

function agent(id: string, extra: Record<string, any> = {}): AgentSpec {
  return {
    id,
    def_name: "test_agent",
    inputs: ["in"],
    outputs: ["out"],
    configs: {},
    x: 0,
    y: 0,
    ...extra,
  };
}

function conn(source: string, target: string, extra: Partial<ConnectionSpec> = {}): ConnectionSpec {
  return { source, source_handle: "out", target, target_handle: "in", ...extra };
}

function flow(agents: AgentSpec[], connections: ConnectionSpec[] = []): PresetFlow {
  const info: PresetInfo = { id: "preset-1", name: "Test", running: false };
  const spec: PresetSpec = { agents, connections, viewport: null };
  return presetToFlow(info, spec);
}

describe("connKey", () => {
  it("distinguishes connections by every tuple element", () => {
    const base = connKey("a", "out", "b", "in");
    expect(connKey("a", "out", "b", "in")).toBe(base);
    expect(connKey("a", "out2", "b", "in")).not.toBe(base);
    expect(connKey("a", "out", "b", "config:in")).not.toBe(base);
    expect(connKey("a2", "out", "b", "in")).not.toBe(base);
    expect(connKey("a", "out", "b2", "in")).not.toBe(base);
  });

  it("treats null and undefined handles alike", () => {
    expect(connKey("a", null, "b", undefined)).toBe(connKey("a", undefined, "b", null));
  });
});

describe("reconcileFlow", () => {
  it("returns identical arrays and objects when nothing changed", () => {
    const cur = flow([agent("a"), agent("b")], [conn("a", "b")]);
    const target = flow([agent("a"), agent("b")], [conn("a", "b")]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(false);
    expect(result.nodes).toBe(cur.nodes);
    expect(result.edges).toBe(cur.edges);
    expect(result.nodes[0]).toBe(cur.nodes[0]);
    expect(result.edges[0]).toBe(cur.edges[0]);
    expect(result.removedAgentIds.size).toBe(0);
    expect(result.removedConnKeys.size).toBe(0);
  });

  it("appends added nodes and edges at the end", () => {
    const cur = flow([agent("a")]);
    const target = flow([agent("b"), agent("a")], [conn("a", "b")]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(true);
    expect(result.nodes.map((n) => n.id)).toEqual(["a", "b"]);
    expect(result.nodes[0]).toBe(cur.nodes[0]);
    expect(result.edges).toHaveLength(1);
    expect(result.edges[0].source).toBe("a");
    expect(result.removedAgentIds.size).toBe(0);
    expect(result.removedConnKeys.size).toBe(0);
  });

  it("removes nodes and edges and reports removed sets", () => {
    const cur = flow([agent("a"), agent("b")], [conn("a", "b")]);
    const target = flow([agent("a")]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(true);
    expect(result.nodes.map((n) => n.id)).toEqual(["a"]);
    expect(result.nodes[0]).toBe(cur.nodes[0]);
    expect(result.edges).toHaveLength(0);
    expect(result.removedAgentIds).toEqual(new Set(["b"]));
    expect(result.removedConnKeys).toEqual(new Set([connKey("a", "out", "b", "in")]));
  });

  it("applies a geometry-only change with a new node object", () => {
    const cur = flow([agent("a")]);
    const target = flow([agent("a", { x: 100, y: 50, width: 3, height: 2 })]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(true);
    expect(result.nodes[0]).not.toBe(cur.nodes[0]);
    expect(result.nodes[0].position).toEqual({ x: 100, y: 50 });
    expect(result.nodes[0].width).toBe(3);
    expect(result.nodes[0].height).toBe(2);
  });

  it("applies a data-only change and keeps position", () => {
    const cur = flow([agent("a")]);
    const target = flow([agent("a", { configs: { key: "value" }, title: "Renamed" })]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(true);
    expect(result.nodes[0]).not.toBe(cur.nodes[0]);
    expect(result.nodes[0].data.configs).toEqual({ key: "value" });
    expect(result.nodes[0].data.title).toBe("Renamed");
    expect(result.nodes[0].position).toEqual({ x: 0, y: 0 });
  });

  it("keeps identity when only the data copy of position is stale", () => {
    // A local move updates node.position (and the backend) but not the
    // x/y mirrored inside data — that difference must not count as a change.
    const cur = flow([agent("a")]);
    cur.nodes[0] = { ...cur.nodes[0], position: { x: 100, y: 50 } };
    const target = flow([agent("a", { x: 100, y: 50 })]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(false);
    expect(result.nodes[0]).toBe(cur.nodes[0]);
  });

  it("keeps identity when only the data copy of width/height is stale", () => {
    const cur = flow([agent("a", { width: 2, height: 1 })]);
    cur.nodes[0] = { ...cur.nodes[0], width: 3, height: 2 };
    const target = flow([agent("a", { width: 3, height: 2 })]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(false);
    expect(result.nodes[0]).toBe(cur.nodes[0]);
  });

  it("still applies non-geometry data changes when the data geometry is stale", () => {
    const cur = flow([agent("a")]);
    cur.nodes[0] = { ...cur.nodes[0], position: { x: 100, y: 0 } };
    const target = flow([agent("a", { x: 100, configs: { key: "v" } })]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(true);
    expect(result.nodes[0].data.configs).toEqual({ key: "v" });
    // The data replacement also refreshes the mirrored geometry
    expect(result.nodes[0].data.x).toBe(100);
  });

  it("preserves selected on changed nodes and edges", () => {
    const cur = flow([agent("a"), agent("b")], [conn("a", "b")]);
    cur.nodes[0].selected = true;
    cur.edges[0].selected = true;
    const target = flow(
      [agent("a", { x: 10, port_colors: { out: 3 } }), agent("b")],
      [conn("a", "b")],
    );

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(true);
    expect(result.nodes[0].selected).toBe(true);
    expect(result.edges[0].selected).toBe(true);
  });

  it("propagates port_colors to the surviving edge style, keeping its id", () => {
    const cur = flow([agent("a"), agent("b")], [conn("a", "b")]);
    const target = flow([agent("a", { port_colors: { out: 3 } }), agent("b")], [conn("a", "b")]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(true);
    expect(result.edges[0]).not.toBe(cur.edges[0]);
    expect(result.edges[0].id).toBe(cur.edges[0].id);
    expect(result.edges[0].style).toContain("var(--color-agent-3)");
  });

  it("identifies edges by their full tuple, not endpoints alone", () => {
    const cur = flow([agent("a"), agent("b", { configs: { key: 1 } })], [conn("a", "b")]);
    const target = flow(
      [agent("a"), agent("b", { configs: { key: 1 } })],
      [conn("a", "b", { target_handle: "config:key" })],
    );

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.changed).toBe(true);
    expect(result.edges).toHaveLength(1);
    expect(result.edges[0].targetHandle).toBe("config:key");
    expect(result.edges[0].id).not.toBe(cur.edges[0].id);
    expect(result.removedConnKeys).toEqual(new Set([connKey("a", "out", "b", "in")]));
  });

  it("keeps surviving node order and appends additions after it", () => {
    const cur = flow([agent("a"), agent("b")]);
    const target = flow([agent("b"), agent("x"), agent("a")]);

    const result = reconcileFlow(cur.nodes, cur.edges, target);

    expect(result.nodes.map((n) => n.id)).toEqual(["a", "b", "x"]);
  });
});
