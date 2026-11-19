import { Code } from "@connectrpc/connect";

import { AlreadySetUp } from "@/features/auth/components/already-set-up";
import { SetupForm } from "@/features/auth/components/setup-form";
import { useSetup } from "@/features/auth/data/use-setup";
import { isCode } from "@/shared/lib/connect-errors";

/** The page that creates the first Admin, with the Setup Code the Server printed to its log. */
export function Setup() {
  const setup = useSetup();
  // Someone else finished setting it up while this page was open.
  return isCode(setup.error, Code.FailedPrecondition) ? (
    <AlreadySetUp />
  ) : (
    <SetupForm setup={setup} />
  );
}
