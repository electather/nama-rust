import { fireEvent, screen, within } from "@testing-library/react";

/** Opens the player's Audio and subtitles panel, unless it's open. */
export function openTracks() {
  if (!screen.queryByRole("dialog", { name: "Audio and subtitles" })) {
    fireEvent.click(screen.getByRole("button", { name: "Audio and subtitles" }));
  }
}

/** Opens Playback settings at its row named `row`, such as "Playback Limit". */
export function openSettingsAt(row: "Speed" | "Playback Limit" | "Playback info") {
  if (!screen.queryByRole("dialog", { name: "Playback settings" })) {
    fireEvent.click(screen.getByRole("button", { name: "Playback settings" }));
  }
  fireEvent.click(screen.getByRole("button", { name: new RegExp(`^${row}`, "u") }));
}

/** The rows of the open list named `list`, "Audio", "Subtitles" or "Playback Limit", or those named `name`. */
export function rowsOf(list: string, name?: RegExp | string) {
  return within(screen.getByRole("radiogroup", { name: list })).getAllByRole(
    "radio",
    name === undefined ? {} : { name },
  );
}

/** The row named `name` of the open list named `list`. */
export function rowIn(list: string, name: RegExp | string) {
  return within(screen.getByRole("radiogroup", { name: list })).getByRole("radio", { name });
}

/** Picks the row named `name` of the list named `list`, opening its panel first. */
export function pickIn(list: "Audio" | "Subtitles" | "Playback Limit", name: RegExp | string) {
  if (list === "Playback Limit") {
    openSettingsAt(list);
  } else {
    openTracks();
  }
  fireEvent.click(
    within(screen.getByRole("radiogroup", { name: list })).getByRole("radio", { name }),
  );
}
