import type { Snippet } from "svelte";
import type { HTMLAttributes, HTMLButtonAttributes } from "svelte/elements";

import type { WithChildren, WithoutChildren } from "bits-ui";

export type PresetFileListRootProps = HTMLAttributes<HTMLDivElement>;

export type PresetFileListFolderProps = WithChildren<{
  name: string;
  open?: boolean;
  class?: string;
  icon?: Snippet<[{ name: string; open: boolean }]>;
  onclick?: (event: MouseEvent) => void;
}>;

export type PresetFileListFilePropsWithoutHTML = WithChildren<{
  name: string;
  icon?: Snippet<[{ name: string }]>;
}>;

export type PresetFileListFileProps = WithoutChildren<HTMLButtonAttributes> &
  PresetFileListFilePropsWithoutHTML;
