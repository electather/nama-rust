import { Headshot } from "@/features/title/components/headshot";
import { PersonTitles } from "@/features/title/components/person-titles";
import { usePerson } from "@/features/title/data/use-person";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Page } from "@/shared/components/page";

/** A Person: their headshot and name, over the titles on this Server they play in, then work on. */
export function PersonPage({ personId }: { personId: ExternalId }) {
  const person = usePerson({ personId });
  return (
    <Page heading={person.name} art={<Headshot url={person.headshotUrl} />}>
      <PersonTitles heading="Cast" roles={person.cast} />
      <PersonTitles heading="Crew" roles={person.crew} />
    </Page>
  );
}
