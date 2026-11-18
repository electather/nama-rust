import { useQuery } from "@connectrpc/connect-query";
import { useId } from "react";

import {
  BrowseService,
  type HomeRow as HomeRowMessage,
  type HomeRowTitle,
  type MovieInLibrary,
  type ShowInLibrary,
} from "@/gen/nama/v1/browse_pb";
import { PosterGrid } from "@/shared/components/poster/poster-grid";
import { MovieInLibraryItem } from "@/shared/components/titles-in-library/movie-in-library-item";
import { ShowInLibraryItem } from "@/shared/components/titles-in-library/show-in-library-item";
import { titleParam } from "@/shared/lib/title-param";

/** A row's one page on home: whole rows of posters at two and five columns. */
const rowPage = { size: 10 };

/**
 * The Home rows Plugins give, after the Kernel's, each loading on its own. Home never waits on
 * them, and shows nothing for one that fails: a Plugin's failures belong on the Plugins page.
 */
export function HomeRows() {
  const { data } = useQuery(BrowseService.method.listHomeRows, {});
  return data?.rows.map((row) => <HomeRow key={`${row.pluginId}/${row.key}`} row={row} />);
}

/** One Plugin's row under its heading, or nothing until its titles load or if it has none. */
function HomeRow({ row }: { row: HomeRowMessage }) {
  const headingId = useId();
  const { data } = useQuery(BrowseService.method.listHomeRowTitles, {
    pluginId: row.pluginId,
    key: row.key,
    page: rowPage,
  });
  const items = data?.titles.flatMap(titleItem) ?? [];
  if (items.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby={headingId} className="mt-10">
      <h2 id={headingId} className="font-medium">
        {row.heading}
      </h2>
      <PosterGrid empty={null}>{items}</PosterGrid>
    </section>
  );
}

function titleItem({ movieOrShow }: HomeRowTitle) {
  const { case: kind, value } = movieOrShow;
  if (kind === "movie") {
    return [<MovieInLibraryItem key={key(value)} movie={value} />];
  }
  if (kind === "show") {
    return [<ShowInLibraryItem key={key(value)} show={value} />];
  }
  return [];
}

/** A title's key in its row, which lists it once, in the one Library it names. */
function key(title: MovieInLibrary | ShowInLibrary) {
  return `${String(title.libraryId)}/${titleParam(title.id)}`;
}
