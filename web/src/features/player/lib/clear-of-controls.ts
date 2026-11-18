/**
 * Ends the cues the file doesn't place 85% of the way down the video rather than at its bottom,
 * where the controls over the bottom of the video would cover them.
 */
export function clearOfControls({ cues }: TextTrack) {
  const unplaced = [...(cues ?? [])].filter(
    (cue): cue is VTTCue => cue instanceof VTTCue && cue.line === "auto",
  );
  for (const cue of unplaced) {
    Object.assign(cue, { snapToLines: false, line: 85, lineAlign: "end" });
  }
}
