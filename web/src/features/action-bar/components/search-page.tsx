import { Results } from "@/features/action-bar/components/results";
import { Page } from "@/shared/components/page";

/** What the nav's search box found in the signed-in User's Libraries for `query`. */
export function SearchPage({ query }: { query: string | undefined }) {
  return (
    <Page heading={query === undefined ? "Search" : `Results for “${query}”`}>
      {query === undefined ? (
        <p className="mt-8 text-muted-foreground">
          Type in the search box to find a Movie, Show or Episode in your Libraries.
        </p>
      ) : (
        <Results query={query} />
      )}
    </Page>
  );
}
