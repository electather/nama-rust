import { useQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

/** The Server's name and version. A placeholder home page until the real one lands. */
export function ServerInfo() {
  const { data, isPending, isError } = useQuery(ServerService.method.getServerInfo);
  return (
    <main className="mx-auto max-w-prose px-6 py-16">
      {isPending && <p className="text-muted-foreground">Loading…</p>}
      {isError && <p>Couldn’t reach this Server. Check that it’s running, then reload.</p>}
      {data && (
        <>
          <h1 className="text-3xl font-semibold">{data.name}</h1>
          <p className="mt-2 text-muted-foreground">Nama {data.version}</p>
        </>
      )}
    </main>
  );
}
