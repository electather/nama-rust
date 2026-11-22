import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

/** Says no Plugin connects Users yet, and who can change that: you, if you manage the Plugins. */
export function NoConnections({ managesPlugins }: { managesPlugins: boolean }) {
  return (
    <div className="flex flex-col items-start gap-2.5 text-sm text-muted-foreground">
      <p>No Plugins on this Server can message you yet.</p>
      {managesPlugins ? (
        <Link
          to="."
          search={(rest) => ({ ...rest, settings: "plugins" })}
          replace
          className={buttonVariants({ variant: "secondary" })}
        >
          See Plugins
        </Link>
      ) : (
        <p>Ask an Admin to add one.</p>
      )}
    </div>
  );
}
