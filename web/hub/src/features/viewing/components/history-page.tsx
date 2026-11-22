import { ViewingHistory } from "@hub/features/viewing/components/viewing-history";
import { Link } from "@tanstack/react-router";

/** The signed-in Account's viewing history: what its Linked Users played, kept as long as it is. */
export function HistoryPage() {
  return (
    <>
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">
        Viewing history
      </h1>
      <p className="text-sm text-muted-foreground">
        Each Movie and Episode your Linked Users play, latest first, kept as long as your Account.
        Titles go by their id at a title source, such as tmdb-603: the Hub keeps no names.
      </p>
      <ViewingHistory />
      <Link to="/" className="text-center text-sm text-primary underline underline-offset-3">
        Back to your Account
      </Link>
    </>
  );
}
