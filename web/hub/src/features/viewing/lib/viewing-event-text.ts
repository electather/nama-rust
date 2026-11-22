import { toJsonString } from "@bufbuild/protobuf";
import {
  DeviceKind,
  type ListViewingEventsResponse,
  ListViewingEventsResponseSchema,
  TitleKind,
  type ViewingEvent,
} from "@hub/gen/nama/hub/v1/viewing_pb";

const startedAt = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

const devices: Record<DeviceKind, string | undefined> = {
  [DeviceKind.UNSPECIFIED]: undefined,
  [DeviceKind.COMPUTER]: "a computer",
  [DeviceKind.PHONE]: "a phone",
  [DeviceKind.TABLET]: "a tablet",
  [DeviceKind.TV]: "a TV",
};

/** What a play played, such as "Movie tmdb-603": the Hub keeps no title's name (ADR 0094). */
export function playedTitle(event: ViewingEvent) {
  const kind = event.kind === TitleKind.EPISODE ? "Episode" : "Movie";
  return `${kind} ${event.externalId}`;
}

/** When a play started, in the viewer's time, and on what kind of Device, if its Server could tell. */
export function playedWhen(event: ViewingEvent) {
  const when = startedAt.format(new Date(Number(event.startedAt) * 1000));
  const device = devices[event.deviceKind];
  return device === undefined ? when : `${when} on ${device}`;
}

/** How far a play got, such as "Finished" or "Stopped at 1:02:03 of 2:16:00". */
export function playedHowFar(event: ViewingEvent) {
  return event.finished
    ? "Finished"
    : `Stopped at ${clock(event.stopPositionMs)} of ${clock(event.durationMs)}`;
}

/** Milliseconds as a clock, such as 1:02:03 or 4:05. */
function clock(ms: bigint) {
  const seconds = Number(ms / 1000n);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor(seconds / 60) % 60;
  const minutesSeconds = `${twoDigits(minutes)}:${twoDigits(seconds % 60)}`;
  return hours > 0 ? `${hours}:${minutesSeconds}` : minutesSeconds.replace(/^0/u, "");
}

function twoDigits(part: number) {
  return part.toString().padStart(2, "0");
}

/** Every Viewing Event as the JSON the Hub API speaks, for an Account to keep. */
export function exportedHistory(history: ListViewingEventsResponse) {
  const json = toJsonString(ListViewingEventsResponseSchema, history, { prettySpaces: 2 });
  return new Blob([json], { type: "application/json" });
}
