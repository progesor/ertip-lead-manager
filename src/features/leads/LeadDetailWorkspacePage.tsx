import { useState } from "react";
import { useLocation, useParams } from "react-router-dom";
import { FollowUpPanel } from "./FollowUpPanel";
import { LeadDetailPage } from "./LeadDetailPage";

interface LeadDetailNavigationState {
  returnTo?: string;
  returnLabel?: string;
  returnState?: unknown;
}

export function LeadDetailWorkspacePage() {
  const { leadId } = useParams();
  const location = useLocation();
  const [revision, setRevision] = useState(0);
  const navigationState = location.state as LeadDetailNavigationState | null;

  return (
    <LeadDetailPage
      key={`lead-detail-${revision}`}
      backTo={navigationState?.returnTo ?? "/leads"}
      backLabel={navigationState?.returnLabel ?? "Leadlere Dön"}
      backState={navigationState?.returnState}
      followUpPanel={
        leadId ? (
          <FollowUpPanel
            contactId={leadId}
            onChanged={() => setRevision((value) => value + 1)}
          />
        ) : null
      }
    />
  );
}
