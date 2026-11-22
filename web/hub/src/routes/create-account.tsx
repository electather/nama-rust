import { CreateAccountPage } from "@hub/features/account/components/create-account-page";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/create-account")({ component: CreateAccountPage });
