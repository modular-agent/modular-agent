import type { Snippet } from "svelte";
import type { HTMLAttributes, HTMLButtonAttributes } from "svelte/elements";

import type { WithChildren, WithoutChildren } from "bits-ui";

export type PresetFileListRootProps = HTMLAttributes<HTMLDivElement>;

export type PresetFileListFolderProps = WithChildren<{
  name: string;
  open?: boolean;
  class?: string;
  draggable?: boolean;
  droptarget?: boolean;
  icon?: Snippet<[{ name: string; open: boolean }]>;
  onclick?: (event: MouseEvent) => void;
  ondragstart?: (event: DragEvent) => void;
  ondragenter?: (event: DragEvent) => void;
  ondragleave?: (event: DragEvent) => void;
  ondrop?: (event: DragEvent) => void;
}>;

export type PresetFileListFilePropsWithoutHTML = WithChildren<{
  name: string;
  icon?: Snippet<[{ name: string }]>;
}>;

export type PresetFileListFileProps = WithoutChildren<HTMLButtonAttributes> &
  PresetFileListFilePropsWithoutHTML;
