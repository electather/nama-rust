import { Found } from "@/features/requests/components/found";
import { useSearchShows } from "@/features/requests/data/use-search-shows";
import { ShowCard } from "@/shared/components/title-cards/show-card";
import { titleParam } from "@/shared/lib/title-param";

/** The Shows found for `title`, each opening its page to pick Seasons. */
export function ShowsFound({ title }: { title: string }) {
  const search = useSearchShows(title);
  return (
    <Found kind="Shows" title={title} search={search}>
      {search.data?.shows.map((show) => (
        <ShowCard key={titleParam(show.id)} show={show} />
      ))}
    </Found>
  );
}
