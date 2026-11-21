import { TitleKind } from "@/gen/nama/v1/match_pb";

/** Which kinds of title the signed-in User may Request, and so search for. */
export interface Searchable {
  movies: boolean;
  shows: boolean;
}

/** The kind of title to search for, for a User who may Request `kinds`: every kind for both. */
export function searchedKind({ movies, shows }: Searchable): TitleKind {
  if (movies && shows) {
    return TitleKind.UNSPECIFIED;
  }
  return movies ? TitleKind.MOVIE : TitleKind.SHOW;
}
