import type { Playback } from "@/gen/nama/v1/event_pb";
import { HardwareAcceleration, type Play, type TranscodeHardware } from "@/gen/nama/v1/server_pb";
import type { Device, User } from "@/gen/nama/v1/sign_in_pb";
import { clock } from "@/shared/lib/clock";
import { hardwareLabel } from "@/shared/lib/playback-names";
import { methodName } from "@/shared/lib/playback-wording";

/** A play with the Playback it plays, which the Server always sends. */
export type KnownPlay = Play & { playback: Playback };

/** Whether `play` carries its Playback, so the page can show it. */
export function hasPlayback(play: Play): play is KnownPlay {
  return play.playback !== undefined;
}

/** How `play` plays, and where its Transcode runs, such as "Transcode on the CPU". */
export function howItPlays({ playback, transcodesOn }: KnownPlay) {
  const method = methodName(playback.method);
  return transcodesOn === undefined ? method : `${method} on ${hardwareWords(transcodesOn)}`;
}

function hardwareWords(hardware: TranscodeHardware) {
  return hardware.api === HardwareAcceleration.OFF ? "the CPU" : hardwareLabel(hardware);
}

/** How far into its Version `play` is, such as "12:34 of 52:05", or `""` till it says. */
export function positionWords({ positionMs, durationMs }: Play) {
  if (positionMs === undefined) {
    return "";
  }
  const at = clock(Number(positionMs) / 1000);
  return durationMs === 0n ? at : `${at} of ${clock(Number(durationMs) / 1000)}`;
}

/**
 * A User on a Device, such as "Ada on Living room TV": the User alone while their Devices load, and
 * "Someone" for a User deleted since.
 */
export function viewerWords(user?: User, device?: Device) {
  const who = user?.name ?? "Someone";
  return device === undefined ? who : `${who} on ${device.name}`;
}
