import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";

import { BackdropGrid, LibraryCard } from "@/components/backdrop-card";
import { ContinueWatching, continueWatchingRow } from "@/components/continue-watching";
import { Page } from "@/components/page";
import { posterLink } from "@/components/poster";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";

export const Route = createFileRoute("/_member/")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query({
        ...createQueryOptions(LibraryService.method.listLibraries, {}, { transport }),
        staleTime: "static",
      }),
      queryClient.query({
        ...createQueryOptions(BrowseService.method.listContinueWatching, continueWatchingRow, {
          transport,
        }),
        staleTime: "static",
      }),
    ]),
  component: Home,
});

function Home() {
  // Fetched already, to know whether the Server needs setting up.
  const { data: server } = useSuspenseQuery(ServerService.method.getServerInfo, {});
  return (
    <Page heading={server.name}>
      <ContinueWatching />
      <section aria-labelledby="libraries" className="mt-10">
        <h2 id="libraries" className="font-medium">
          Libraries
        </h2>
        <Libraries />
      </section>
    </Page>
  );
}

function Libraries() {
  const { data } = useSuspenseQuery(LibraryService.method.listLibraries, {});
  if (data.libraries.length === 0) {
    return (
      <p className="mt-2 text-muted-foreground">
        This Server has no Libraries yet. Once one is added, it shows up here.
      </p>
    );
  }
  return (
    <BackdropGrid>
      {data.libraries.map((library) => (
        <li key={String(library.id)}>
          <Link
            to="/libraries/$libraryId"
            params={{ libraryId: String(library.id) }}
            className={posterLink}
          >
            <LibraryCard library={library} />
          </Link>
        </li>
      ))}
    </BackdropGrid>
  );
}
