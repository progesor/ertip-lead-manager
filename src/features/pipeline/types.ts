import type { LeadStatus, ProductCode } from "../leads/types";

export interface PipelineCard {
  id: string;
  displayName: string;
  primaryEmail: string | null;
  primaryPhone: string | null;
  countryCode: string | null;
  status: LeadStatus;
  latestSubmissionAt: string | null;
  submissionCount: number;
  isRepeat: boolean;
  productInterests: ProductCode[];
  platforms: string[];
  warningCount: number;
  nextFollowUpAt: string | null;
  openFollowUpCount: number;
}

export interface PipelineColumn {
  status: LeadStatus;
  total: number;
  cards: PipelineCard[];
  truncated: boolean;
}

export interface PipelineBoardResponse {
  columns: PipelineColumn[];
  visibleTotal: number;
  perColumnLimit: number;
}
