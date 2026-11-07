import { Code, ConnectError } from "@connectrpc/connect";
import { createFileRoute } from "@tanstack/react-router";

import { Role } from "@/gen/nama/v1/sign_in_pb";

/** Pages for Admins. A Member gets the same "Admins only" page the Server's own refusal shows. */
export const Route = createFileRoute("/_member/_admin")({
  beforeLoad: ({ context: { signedIn } }) => {
    if (signedIn.user?.role !== Role.ADMIN) {
      throw new ConnectError("Admins only", Code.PermissionDenied);
    }
  },
});
