import { SubtitleBackground, SubtitleSize } from "@/gen/nama/v1/playback_pb";

/** Each size a User's text subtitles can be, with its label. */
export const sizes: [size: SubtitleSize, label: string][] = [
  [SubtitleSize.SMALL, "Small"],
  [SubtitleSize.DEFAULT, "Default"],
  [SubtitleSize.LARGE, "Large"],
  [SubtitleSize.LARGEST, "Largest"],
];

/** Each background a User's text subtitles can have, with its label. */
export const backgrounds: [background: SubtitleBackground, label: string][] = [
  [SubtitleBackground.OUTLINE, "Outline, a dark edge around each letter"],
  [SubtitleBackground.BOX, "Box, a dark box behind each line"],
];

/** `choices` as radio buttons' values and labels. */
export function radios(choices: [value: number, label: string][]): [string, string][] {
  return choices.map(([value, label]) => [String(value), label]);
}

/** The one of `choices` whose radio button is valued `radio`, or else 0, an enum's unspecified. */
export function chosen<T extends number>(choices: [T, string][], radio: string): T | 0 {
  return choices.find(([value]) => String(value) === radio)?.[0] ?? 0;
}

/**
 * What part of the appearance shows: the pick `save` is saving, which `pick` takes from its
 * request, until the Server answers, and then `saved`, so a pick the Server refused goes back.
 */
export function shown<Request, T>(
  save: { isPending: boolean; variables: Request | undefined },
  pick: (request: Request) => T,
  saved: T,
): T {
  return save.isPending && save.variables !== undefined ? pick(save.variables) : saved;
}
