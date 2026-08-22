import { useState } from "react";
import { useParams } from "react-router-dom";
import { FollowUpPanel } from "./FollowUpPanel";
import { LeadDetailPage } from "./LeadDetailPage";

export function LeadDetailWorkspacePage() {
  const { leadId } = useParams();
  const [revision, setRevision] = useState(0);

  return (
    <>
      <LeadDetailPage key={`lead-detail-${revision}`} />
      {leadId ? (
        <FollowUpPanel
          contactId={leadId}
          onChanged={() => setRevision((value) => value + 1)}
        />
      ) : null}
    </>
  );
}
