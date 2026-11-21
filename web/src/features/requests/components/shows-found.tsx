import { Found } from "@/features/requests/components/found";
import type { ShowHit } from "@/gen/nama/v1/request_pb";
import type { TitleStatus } from "@/gen/nama/v1/search_pb";
import { ShowCard } from "@/shared/components/title-cards/show-card";
import { titleParam } from "@/shared/lib/title-param";

/** The Shows a `search` for `title` found, each opening its page to watch or pick Seasons. */
export function ShowsFound({
  title,
  search,
}: {
  title: string;
  search: { shows: { show: ShowHit; status: TitleStatus }[]; searching: boolean; failed: boolean };
}) {
  return (
    <Found kind="Shows" title={title} search={search}>
      {search.shows.map(({ show, status }) => (
        <ShowCard key={titleParam(show.id)} show={show} status={status} />
      ))}
    </Found>
  );
}
