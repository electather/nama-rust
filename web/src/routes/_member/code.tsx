import { createFileRoute } from "@tanstack/react-router";

import { EnterCode } from "@/features/auth/components/enter-code";

export const Route = createFileRoute("/_member/code")({
  // `?code=` is what a phone opens by scanning the QR code a signing-in Device shows.
  validateSearch: (search): { code?: string | undefined } => ({
    code: typeof search["code"] === "string" ? search["code"] : undefined,
  }),
  component: CodeRoute,
});

function CodeRoute() {
  const { code } = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <EnterCode
      scanned={code}
      onStartOver={() => {
        // Otherwise the next code's field would start with this one.
        void navigate({ search: {} });
      }}
    />
  );
}
