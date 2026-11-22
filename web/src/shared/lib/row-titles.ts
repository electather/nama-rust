import type { HomeRowTitle } from "@/gen/nama/v1/browse_pb";
import { titleParam } from "@/shared/lib/title-param";

/** A row's title: a Movie or Show on the Server, or one missing from it. */
export type RowTitle = Exclude<HomeRowTitle["movieOrShow"], { case: undefined }>;

/** The titles of a row this app knows how to show. */
export function rowTitles(titles: HomeRowTitle[]): RowTitle[] {
  return titles.flatMap(({ movieOrShow }) => (movieOrShow.case === undefined ? [] : [movieOrShow]));
}

/** A title's key in its row, which lists it once: in the one Library it names, or as missing. */
export function rowTitleKey(title: RowTitle) {
  const where =
    title.case === "movie" || title.case === "show" ? String(title.value.libraryId) : title.case;
  return `${where}/${titleParam(title.value.id)}`;
}
