import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { OneClickRequest } from "@/shared/components/request-button/one-click-request";
import { QualityMenuLoader } from "@/shared/components/request-button/quality-menu-loader";
import { useMay } from "@/shared/hooks/use-may";
import { ButtonGroup } from "@/shared/ui/button-group";

/**
 * Request, which asks for `what` at the User's Quality, and beside it a chevron whose menu asks at
 * one of the Qualities of the Fulfiller for `kind`. The chevron shows only while `opening` a Request,
 * since joining one keeps its Quality, to a User who may choose, when there's a choice. Request is
 * the screen's primary action if `primary`, and `busy` while it's being sent.
 */
export function RequestSplitButton({
  what,
  kind,
  opening,
  primary,
  busy,
  onRequest,
}: {
  what: string;
  kind: "movie" | "show";
  opening: boolean;
  primary: boolean;
  busy: boolean;
  onRequest: (quality?: bigint) => void;
}) {
  const mayChoose = useMay(Permission.CHOOSE_QUALITY);
  // A ghost button's icon lines up with the caption above it, its padding hanging outside.
  const [variant, hanging] = primary ? (["default"] as const) : (["ghost", "-ms-4"] as const);
  const bothParts = { what, variant, disabled: busy };
  return (
    <span className={hanging}>
      <ButtonGroup>
        <OneClickRequest {...bothParts} onRequest={onRequest} />
        {opening && mayChoose && (
          <QualityMenuLoader {...bothParts} kind={kind} onPick={onRequest} />
        )}
      </ButtonGroup>
    </span>
  );
}
