import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useRecordSignedIn } from "@hub/features/account/data/account";
import { AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { HubService } from "@hub/gen/nama/hub/v1/hub_pb";
import { useRouter } from "@tanstack/react-router";

/** Whether the Hub emails Sign-in Links; `false` until it has said so. */
export function useSendsSignInLinks() {
  const { data } = useQuery(HubService.method.getHubInfo, {});
  return data?.signInLinks ?? false;
}

/** Asks the Hub to email a Sign-in Link. */
export function useSendSignInLink() {
  return useMutation(AccountService.method.sendSignInLink);
}

/** The Account the Sign-in Link `token` signs in, without using it up. */
export function useSignInLink(token: string) {
  return useQuery(AccountService.method.getSignInLink, { token });
}

/** Signs in with a Sign-in Link, then goes where the sign-in continues. */
export function useSignInWithLink() {
  const router = useRouter();
  const recordSignedIn = useRecordSignedIn();
  return useMutation(AccountService.method.signInWithLink, {
    onSuccess: async ({ account, continueTo }) => {
      // Not clearing the cache: that would look the used link up again, which now says it expired.
      recordSignedIn(account);
      await router.navigate({ href: continueTo });
    },
  });
}
