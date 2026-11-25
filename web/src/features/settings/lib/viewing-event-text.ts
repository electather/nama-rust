import { DeviceKind, type ViewingEvent } from "@/gen/nama/hub/v1/viewing_pb";
import { clock } from "@/shared/lib/clock";

const devices: Record<DeviceKind, string | undefined> = {
  [DeviceKind.UNSPECIFIED]: undefined,
  [DeviceKind.COMPUTER]: "a computer",
  [DeviceKind.PHONE]: "a phone",
  [DeviceKind.TABLET]: "a tablet",
  [DeviceKind.TV]: "a TV",
};

/** What a play played, such as "Movie tmdb-603": the Hub keeps no title's name (ADR 0094). */
export function playedTitle(event: ViewingEvent) {
  return `${event.episode ? "Episode" : "Movie"} ${event.externalId}`;
}

/** When a play started, in this browser's time, and on what kind of Device, if its Server could tell. */
export function playedWhen(event: ViewingEvent) {
  const started = new Date(Number(event.startedAt) * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
  const device = devices[event.deviceKind];
  return device === undefined ? started : `${started} on ${device}`;
}

/** How far a play got, such as "Finished" or "Stopped at 1:02:03 of 2:16:00". */
export function playedHowFar({ finished, stopPositionMs, durationMs }: ViewingEvent) {
  return finished
    ? "Finished"
    : `Stopped at ${clock(seconds(stopPositionMs))} of ${clock(seconds(durationMs))}`;
}

function seconds(ms: bigint) {
  return Number(ms / 1000n);
}
