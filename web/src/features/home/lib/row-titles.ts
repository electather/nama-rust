import type { HomeRowTitle, MovieInLibrary, ShowInLibrary } from "@/gen/nama/v1/browse_pb";
import { titleParam } from "@/shared/lib/title-param";

/** A row's Movie or Show. */
export type RowTitle = Extract<HomeRowTitle["movieOrShow"], { case: "movie" | "show" }>;

/** The titles of a row that are a Movie or a Show. */
export function rowTitles(titles: HomeRowTitle[]): RowTitle[] {
  return titles.flatMap(({ movieOrShow }) =>
    movieOrShow.case === "movie" || movieOrShow.case === "show" ? [movieOrShow] : [],
  );
}

/** A title's key in its row, which lists it once, in the one Library it names. */
export function rowTitleKey(title: MovieInLibrary | ShowInLibrary) {
  return `${String(title.libraryId)}/${titleParam(title.id)}`;
}
