import { PlayProgress } from "@/shared/components/backdrop-card/play-progress";

/** A Trailer card's caption: its `name`, then ▶ and its `host`, such as YouTube. */
export function TrailerCaption({ name, host }: { name: string; host: string | undefined }) {
  return (
    <>
      {/* A source's names run long ("Alien | 40th Anniversary Trailer | 20th Century FOX"). */}
      <span className="truncate leading-6 font-semibold">{name}</span>
      <PlayProgress state={undefined} detail={host} />
    </>
  );
}
