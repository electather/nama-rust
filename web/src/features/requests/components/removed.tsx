import { RequestLayout } from "@/features/requests/components/request-layout";

/** What the Request page says when its Request is gone, or isn't one the User can see. */
export function Removed() {
  return (
    <RequestLayout>
      <p className="mt-8 text-muted-foreground">
        This Request was removed, or isn’t one you can see.
      </p>
    </RequestLayout>
  );
}
