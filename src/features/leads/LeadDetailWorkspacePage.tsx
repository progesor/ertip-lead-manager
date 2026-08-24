import { useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { FollowUpPanel } from "./FollowUpPanel";
import { LeadDetailPage } from "./LeadDetailPage";
import "./lead-detail-origin.css";

interface LeadDetailNavigationState {
  returnTo?: string;
  returnLabel?: string;
  returnState?: unknown;
}

export function LeadDetailWorkspacePage() {
  const { leadId } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const [revision, setRevision] = useState(0);
  const navigationState = location.state as LeadDetailNavigationState | null;
  const hasCustomReturn = Boolean(
    navigationState?.returnTo && navigationState.returnTo !== "/leads",
  );

  function returnToOrigin() {
    if (!navigationState?.returnTo) return;
    navigate(navigationState.returnTo, { state: navigationState.returnState });
  }

  return (
    <div className={`lead-detail-workspace ${hasCustomReturn ? "has-custom-return" : ""}`}>
      {hasCustomReturn ? (
        <div className="lead-detail-origin-return">
          <button type="button" className="lead-back-button" onClick={returnToOrigin}>
            ← {navigationState?.returnLabel ?? "Geri Dön"}
          </button>
        </div>
      ) : null}

      <LeadDetailPage key={`lead-detail-${revision}`} />
      {leadId ? (
        <FollowUpPanel
          contactId={leadId}
          onChanged={() => setRevision((value) => value + 1)}
        />
      ) : null}
    </div>
  );
}
