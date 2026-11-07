import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";

import { Page } from "@/components/page";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";

export const Route = createFileRoute("/_member/")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(LibraryService.method.listLibraries, {}, { transport }),
      staleTime: "static",
    }),
  component: Home,
});

// Continue Watching goes above the Libraries, once Watch State has a UI (#73).
function Home() {
  // Fetched already, to know whether the Server needs setting up.
  const { data: server } = useSuspenseQuery(ServerService.method.getServerInfo, {});
  return (
    <Page heading={server.name}>
      <section aria-labelledby="libraries" className="mt-10 max-w-2xl">
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
        This Server has no Libraries yet. Once an Admin adds one, it shows up here.
      </p>
    );
  }
  return (
    // Pulled out by the rows' own padding, so the names line up with the heading.
    <ul className="mt-2 -mx-3">
      {data.libraries.map((library) => (
        <li key={String(library.id)} className="border-b last:border-b-0">
          <Link
            to="/libraries/$libraryId"
            params={{ libraryId: String(library.id) }}
            className="flex min-h-14 items-center gap-3 rounded-lg px-3 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
          >
            <span className="truncate text-lg font-medium">{library.name}</span>
            <span className="ms-auto text-sm text-muted-foreground">
              {library.kind === LibraryKind.SHOWS ? "Shows" : "Movies"}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
}
