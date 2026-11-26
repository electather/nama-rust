import { PeopleLinks } from "@/features/title/components/people-links";
import { TitleDetails } from "@/features/title/components/title-details";
import { originalAudio } from "@/features/title/lib/original-audio";
import { crewWithJob } from "@/features/title/lib/people";
import type { FoundShow } from "@/gen/nama/v1/request_pb";

/** A found Show's Details: who created it and its Original audio. */
export function FoundShowDetails({ show }: { show: FoundShow }) {
  const creators = crewWithJob(show.crew, "Creator");
  return (
    <TitleDetails
      groups={[
        {
          heading: "Information",
          facts: [["Created by", creators && <PeopleLinks key="crew" people={creators} />]],
        },
        { heading: "Languages", facts: [originalAudio(show.originalLanguage)] },
      ]}
    />
  );
}
