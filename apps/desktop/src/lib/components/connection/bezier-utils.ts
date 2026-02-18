import { Position } from "@xyflow/svelte";

/** Control point offset factor for positive distance (xyflow default: 0.5) */
export const CONTROL_OFFSET_FACTOR = 0.5;

export function controlOffset(distance: number): number {
  if (distance < 0) distance = -distance;
  return CONTROL_OFFSET_FACTOR * 25 * Math.sqrt(distance);
}

export function controlPoint(
  pos: Position,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
): [number, number] {
  switch (pos) {
    case Position.Left:
      return [x1 - controlOffset(x1 - x2), y1];
    case Position.Right:
      return [x1 + controlOffset(x2 - x1), y1];
    case Position.Top:
      return [x1, y1 - controlOffset(y1 - y2)];
    case Position.Bottom:
      return [x1, y1 + controlOffset(y2 - y1)];
    default:
      return [x1, y1];
  }
}
