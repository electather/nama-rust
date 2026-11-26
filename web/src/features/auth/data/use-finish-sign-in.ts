import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

/** Once someone is signed in: forgets what was cached before, and goes to `redirect` or home. */
export function useFinishSignIn(redirect: string | undefined) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  return async () => {
    // Forget the signed-out visitor, and anything cached for whoever was signed in before.
    queryClient.clear();
    // In the sign-in page's place: going back to it would only send Back on to `redirect` again.
    await navigate({ href: redirect ?? "/", replace: true });
  };
}
