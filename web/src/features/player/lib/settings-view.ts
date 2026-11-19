/** What the Playback settings panel shows: its rows, one of the lists they open, or Playback info. */
export type SettingsView = "settings" | "speed" | "limit" | "info";

/** Whether a close, for `reason`, goes back to the rows instead: Esc in a list or Playback info. */
export function stepsBack(open: boolean, reason: string, view: SettingsView) {
  return !open && reason === "escape-key" && view !== "settings";
}

/** Which side a page of Playback settings slides in from: a list from the end, the rows from the start. */
export type SlideFrom = "start" | "end";

// The new page slides a little way in as it fades up, while the panel eases to its size; the old
// one is gone at once, as the two drawn over each other in a resizing panel read as a smear.
const slides = {
  start:
    "animate-in fade-in slide-in-from-start-6 duration-250 ease-out-expo motion-reduce:animate-none",
  end: "animate-in fade-in slide-in-from-end-6 duration-250 ease-out-expo motion-reduce:animate-none",
};

/** The classes sliding a page in `from` a side, or none for the page the panel opens on. */
export function slideIn(from: SlideFrom | undefined) {
  return from === undefined ? undefined : slides[from];
}
