import { useQuery } from "@connectrpc/connect-query";

import { Page } from "@/components/page";
import { ServerService } from "@/gen/nama/v1/server_pb";

/** The Server's name and version. A placeholder home page until the real one lands. */
export function ServerInfo() {
  const { data, isError } = useQuery(ServerService.method.getServerInfo);
  return (
    <Page heading={data?.name ?? "This Server"}>
      <p className="mt-2 text-muted-foreground">
        {data && `Nama ${data.version}`}
        {!data &&
          (isError
            ? "Couldn’t reach this Server. Check that it’s running, then reload."
            : "Loading…")}
      </p>
    </Page>
  );
}
