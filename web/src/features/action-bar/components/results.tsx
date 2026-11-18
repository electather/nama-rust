import { EpisodeList } from "@/features/action-bar/components/episode-list";
import { FoundSection } from "@/features/action-bar/components/found-section";
import { MovieGrid } from "@/features/action-bar/components/movie-grid";
import { ShowGrid } from "@/features/action-bar/components/show-grid";
import { useSearchResults } from "@/features/action-bar/data/use-search-results";
import { ShowMore } from "@/shared/components/show-more";

/** What a search for `query` found, a page at a time, grouped by kind, or that it found nothing. */
export function Results({ query }: { query: string }) {
  const { data, hasNextPage, fetchNextPage } = useSearchResults(query);
  const movies = data.pages.flatMap((page) => page.movies);
  const shows = data.pages.flatMap((page) => page.shows);
  const episodes = data.pages.flatMap((page) => page.episodes);
  if (movies.length + shows.length + episodes.length === 0) {
    return <p className="mt-8 text-muted-foreground">Nothing found for “{query}”.</p>;
  }
  // A Movie, Show or Episode in two Libraries is found in each, so keys name the Library too.
  return (
    <>
      <FoundSection heading="Movies" count={movies.length}>
        <MovieGrid movies={movies} />
      </FoundSection>
      <FoundSection heading="Shows" count={shows.length}>
        <ShowGrid shows={shows} />
      </FoundSection>
      <FoundSection heading="Episodes" count={episodes.length}>
        <EpisodeList episodes={episodes} />
      </FoundSection>
      {hasNextPage && <ShowMore load={fetchNextPage} loaded={data.pages.length} />}
    </>
  );
}
