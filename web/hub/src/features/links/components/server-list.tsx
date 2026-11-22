import { LinkedServers } from "@hub/features/links/components/linked-servers";

/** The Servers the signed-in Account owns or has a User on, each with when it was Linked. */
export function ServerList() {
  return (
    <section aria-labelledby="your-servers" className="flex flex-col gap-2">
      <h2 id="your-servers" className="text-lg/6 font-medium">
        Your Servers
      </h2>
      <LinkedServers />
    </section>
  );
}
