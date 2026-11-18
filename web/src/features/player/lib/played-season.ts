import type { ExternalId } from "@/gen/nama/v1/match_pb";

/** The Season of a Show whose Episode plays, in the Library that has it. */
export interface PlayedSeason {
  libraryId: bigint;
  showId: ExternalId;
  seasonNumber: number;
}
