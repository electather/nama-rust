/** Each cue's times as its file has them, so shifting it again starts from those. */
const fileTimes = new WeakMap<TextTrackCue, { startTime: number; endTime: number }>();

/** Shows `track`'s cues `offsetMs` later than its file times them, or earlier for a negative one. */
export function shiftCues({ cues }: TextTrack, offsetMs: number) {
  for (const cue of cues ?? []) {
    const file = fileTimes.get(cue) ?? { startTime: cue.startTime, endTime: cue.endTime };
    fileTimes.set(cue, file);
    Object.assign(cue, {
      startTime: file.startTime + offsetMs / 1000,
      endTime: file.endTime + offsetMs / 1000,
    });
  }
}
