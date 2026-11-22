import { SignInPage } from "@hub/features/account/components/sign-in-page";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/sign-in")({ component: SignInPage });
