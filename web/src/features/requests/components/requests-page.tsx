import { FindOrList } from "@/features/requests/components/find-or-list";
import { RequestList } from "@/features/requests/components/request-list";
import type { StateFilter } from "@/features/requests/lib/state-filter";
import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { Page } from "@/shared/components/page";
import { may } from "@/shared/lib/permissions";

/** The Requests page: the search for a title to Request, if the User may, and the Requests listed under the `filter` they chose. */
export function RequestsPage({
  user,
  title,
  filter,
  choose,
}: {
  user: User | undefined;
  title: string | undefined;
  filter: StateFilter;
  choose: (state: StateFilter) => void;
}) {
  const kinds = {
    movies: may(user, Permission.REQUEST_MOVIE),
    shows: may(user, Permission.REQUEST_TV),
  };
  return (
    <Page heading="Requests">
      {kinds.movies || kinds.shows ? (
        <FindOrList title={title} kinds={kinds} you={user?.id} filter={filter} choose={choose} />
      ) : (
        <RequestList filter={filter} choose={choose} />
      )}
    </Page>
  );
}
