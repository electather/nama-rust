import { Link } from "@tanstack/react-router";
import { HardDrive } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "nama-ui/components/empty";

import { sectionSearch } from "@/shared/lib/settings-address";

/** Home at the Hub for an Account with no Linked Server: Linking one, in Settings. */
export function LinkServerPrompt() {
  const icon = (
    <EmptyMedia>
      <HardDrive aria-hidden strokeWidth={1.5} />
    </EmptyMedia>
  );
  return (
    <Empty>
      <EmptyHeader>
        {icon}
        <EmptyTitle>No Server is Linked to your Account yet</EmptyTitle>
      </EmptyHeader>
      <Link
        to="."
        search={(rest) => ({ ...rest, ...sectionSearch("account-servers") })}
        className={buttonVariants({ size: "lg" })}
      >
        Link a Server
      </Link>
    </Empty>
  );
}
