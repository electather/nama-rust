import { ContinueWatching } from "@/features/home/components/continue-watching";
import { HomeRows } from "@/features/home/components/home-rows";
import { Libraries } from "@/features/home/components/libraries";
import { useServerInfo } from "@/features/home/data/use-server-info";
import { Page } from "@/shared/components/page";

/** Home: Continue Watching, the Libraries and the rows Plugins give. */
export function HomePage() {
  const server = useServerInfo();
  return (
    <Page heading={server.name}>
      <ContinueWatching />
      <section aria-labelledby="libraries" className="mt-10">
        <h2 id="libraries" className="font-medium">
          Libraries
        </h2>
        <Libraries />
      </section>
      <HomeRows />
    </Page>
  );
}
