import { LinkControls } from "@/features/settings/components/link-controls";
import { useStartUserLink } from "@/features/settings/data/use-start-user-link";
import { useUnlinkUser } from "@/features/settings/data/use-unlink-user";
import { useUserLink } from "@/features/settings/data/use-user-link";
import { userLinkFailure } from "@/features/settings/lib/server-link";

/**
 * Whether the signed-in User is Linked to an Account on the Hub, and Linking or Unlinking them.
 * Shown only once the Server is Linked, as only its Users can Link.
 */
export function UserLink() {
  const link = useUserLink();
  const start = useStartUserLink();
  const unlink = useUnlinkUser();
  if (link === undefined) {
    return null;
  }
  return (
    <section aria-labelledby="user-link" className="flex flex-col gap-3.5">
      <div>
        <h4 id="user-link" className="text-xs text-muted-foreground">
          Hub
        </h4>
        <p className="mt-1 text-sm text-pretty text-muted-foreground">
          Link to your Account on the Hub to take what you watch to other Servers. Without it, you
          use this Server just the same.
        </p>
      </div>
      <LinkControls
        link={link}
        enterUnder="Link a User"
        start={start}
        unlink={unlink}
        failure={userLinkFailure(start.error ?? unlink.error)}
      />
    </section>
  );
}
