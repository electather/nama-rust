import type { Version } from "@/gen/nama/v1/browse_pb";

/** The Version `chosen` names, or the largest when it names none of `versions`. */
export function versionToPlay(versions: Version[], chosen?: number) {
  return versions.find(({ id }) => Number(id) === chosen) ?? largestVersion(versions);
}

/** The Version to play when nobody chose one: the largest file. */
function largestVersion([first, ...others]: Version[]) {
  // The Server sends at least one.
  if (first === undefined) {
    throw new TypeError("a title has at least one Version");
  }
  let largest = first;
  for (const version of others) {
    if (version.size > largest.size) {
      largest = version;
    }
  }
  return largest;
}
