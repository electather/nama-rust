import { type Hardware, hardwareLabel } from "@/features/admin-playback/lib/hardware";
import { methodName } from "@/features/player/lib/playback-wording";
import { clock } from "@/features/player/lib/video-actions";
import { HardwareAcceleration, type Play } from "@/gen/nama/v1/server_pb";
import type { Device, User } from "@/gen/nama/v1/sign_in_pb";

/** How `play` plays, and where its Transcode runs, such as "Transcode on the CPU". */
export function howItPlays({ playback, transcodesOn }: Play) {
  const method = methodName(playback?.method ?? 0);
  return transcodesOn === undefined ? method : `${method} on ${where(transcodesOn)}`;
}

function where(hardware: Hardware) {
  return hardware.api === HardwareAcceleration.OFF ? "the CPU" : hardwareLabel(hardware);
}

/** How far into its Version `play` is, such as "12:34 of 52:05", or `""` till it says. */
export function progress({ positionMs, durationMs }: Play) {
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
