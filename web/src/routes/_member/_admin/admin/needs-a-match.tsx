import { createFileRoute } from "@tanstack/react-router";

import { NeedsAMatch } from "@/features/admin-needs-a-match/components/needs-a-match";
import { unmatchedItemsQuery } from "@/features/admin-needs-a-match/data/use-unmatched-items";
import { librariesQuery } from "@/shared/hooks/use-libraries";

export const Route = createFileRoute("/_member/_admin/admin/needs-a-match")({
  validateSearch: (search: Record<string, unknown>): { library?: string } =>
    typeof search["library"] === "number" || typeof search["library"] === "string"
      ? { library: String(search["library"]) }
      : {},
  loader: async ({ context: { queryClient, transport } }) => {
    const { libraries } = await queryClient.query(librariesQuery(transport));
    await Promise.all(
      libraries.map((library) =>
        queryClient.infiniteQuery(unmatchedItemsQuery(library, transport)),
      ),
    );
  },
  component: NeedsAMatchRoute,
});

function NeedsAMatchRoute() {
  const { library } = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <NeedsAMatch
      picked={library}
      onPick={(picked) => {
        void navigate({ search: picked === undefined ? {} : { library: picked } });
      }}
    />
  );
}
