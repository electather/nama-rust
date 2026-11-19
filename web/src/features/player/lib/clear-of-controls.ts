/** The cues this app placed, so they move again when the controls do, while the file's own stay put. */
const placed = new WeakSet<TextTrackCue>();

/**
 * Ends the cues the file doesn't place `line`% of the way down the video rather than at its bottom:
 * clear of the controls while they're up, and lower once they hide.
 */
export function clearOfControls({ cues }: TextTrack, line: number) {
  const ours = [...(cues ?? [])].filter(
    (cue): cue is VTTCue => cue instanceof VTTCue && (cue.line === "auto" || placed.has(cue)),
  );
  for (const cue of ours) {
    placed.add(cue);
    Object.assign(cue, { snapToLines: false, line, lineAlign: "end" });
  }
}
