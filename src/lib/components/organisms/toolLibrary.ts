// Shared contract of the tool library page (107 S10, S11): the `tool_detail` payload the command
// returns. It lives here, once, because three components read it — the panel that invokes the
// command, the detail form and the preview — and structural typing would let one copy drift from
// another without any of them failing to compile. The repository keeps domain types next to the
// components that share them (`src/lib/components/workbench/workbench.svelte.ts` is the sibling
// case), so this is that home for the library page rather than a new types directory.

/** The `tool_detail` command's payload for one tool of the library. */
export type ToolDetail = {
  schema: string;
  /** The library's stable geometry id; the same one a tree row carries. */
  id: string;
  /** S10's `Name`, rendered from the record by the command. */
  name: string;
  kind: string;
  /** The kind's own word (`Flat end mill`). */
  kindLabel: string;
  number: number | null;
  diameterMm: number | null;
  flutes: number | null;
  cutLengthMm: number | null;
  shankMm: number | null;
  overallLengthMm: number | null;
  includedAngleDeg: number | null;
  flatDiameterMm: number | null;
  spindleRpm: number | null;
  cutFeedMmPerMin: number | null;
  plungeFeedMmPerMin: number | null;
  stepoverMm: number | null;
  /** Absent by design: the model holds no percentage of the diameter. */
  stepoverPercent: number | null;
};
