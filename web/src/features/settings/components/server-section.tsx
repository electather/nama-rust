import { ServerUrl } from "@/features/settings/components/server-url";

/** The Server heading over the form that sets the Server URL. */
export function ServerSection() {
  return (
    <section aria-labelledby="server" className="mt-12 max-w-md">
      <h2 id="server" className="font-medium">
        Server
      </h2>
      <ServerUrl />
    </section>
  );
}
