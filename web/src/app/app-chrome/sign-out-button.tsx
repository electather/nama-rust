import { useSignOut } from "@/app/data/use-sign-out";
import { Button } from "@/shared/ui/button";

/** Signs out of this Device, then opens sign-in. */
export function SignOutButton() {
  const signOut = useSignOut();
  return (
    <Button
      variant="ghost"
      size="bar"
      disabled={signOut.isPending}
      onClick={() => {
        signOut.mutate({});
      }}
    >
      Sign out
    </Button>
  );
}
