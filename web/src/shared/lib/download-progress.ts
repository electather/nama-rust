import type { Download } from "@/gen/nama/v1/request_pb";
import { runtimeOf } from "@/shared/lib/title-facts";

/** Each download's percent weighted by its size, or counted evenly unless every size is known. */
export function overallPercent(downloads: Download[]) {
  const sized = downloads.every(({ sizeBytes }) => sizeBytes !== undefined && sizeBytes > 0n);
  const weight = ({ sizeBytes }: Download) => (sized ? Number(sizeBytes) : 1);
  let total = 0;
  let done = 0;
  for (const download of downloads) {
    total += weight(download);
    done += weight(download) * download.percent;
  }
  return done / total;
}

/** How long `seconds` leave, such as "1 h 30 min left", or nothing once none do. */
export function timeLeftText(seconds: number) {
  const left = runtimeOf(Math.ceil(seconds / 60));
  return left === "" ? undefined : `${left} left`;
}

/** How long the slowest of `downloads` says it has left, or nothing if none says. */
export function slowestTimeLeft(downloads: Download[]) {
  const etas = downloads.flatMap(({ etaSeconds }) =>
    etaSeconds === undefined ? [] : [Number(etaSeconds)],
  );
  return etas.length === 0 ? undefined : timeLeftText(Math.max(...etas));
}
