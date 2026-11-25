import { Suspense } from "react";

import { NoUserNotice } from "@/features/auth/components/no-user-notice";
import { UserInvites } from "@/features/auth/components/user-invites";
import { ErrorBoundary } from "@/shared/components/error-boundary";
import { Notice } from "@/shared/components/sign-in-layout/notice";

const failed = (
  <Notice kind="error">
    The Hub couldn’t list your Users just now. Reload the page to try again.
  </Notice>
);

/**
 * The Users waiting for the signed-in Account to accept them, and, while it has no Linked User,
 * what it can do about that. Linking by code over them works while they load or if they can't.
 */
export function AccountLinks() {
  return (
    <ErrorBoundary failed={failed}>
      <Suspense fallback={null}>
        <UserInvites />
        <NoUserNotice />
      </Suspense>
    </ErrorBoundary>
  );
}
