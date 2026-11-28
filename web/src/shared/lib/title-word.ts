import { TitleKind } from "@/gen/nama/v1/match_pb";

/** "Movie" or "Show": what a file's title is, an Episode's being its Show. */
export function titleWord(kind: TitleKind) {
  return kind === TitleKind.SHOW ? "Show" : "Movie";
}
