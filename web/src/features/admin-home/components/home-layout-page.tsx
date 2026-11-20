import { HomeLayoutForm } from "@/features/admin-home/components/home-layout-form";
import { Page } from "@/shared/components/page";

/** The Home page: the default Home layout every User's Home follows. */
export function HomeLayoutPage() {
  return (
    <Page heading="Home">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        Every User’s Home shows these rows in this order, under Featured. Drag a row, or use its
        arrows, to move it, and switch off what Home shouldn’t show. A row from a Plugin enabled
        later goes at the end.
      </p>
      <HomeLayoutForm />
    </Page>
  );
}
