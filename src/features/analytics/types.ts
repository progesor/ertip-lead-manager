import type { LeadStatus } from "../leads/types";

export interface AnalyticsRequest {
  fromUtc: string | null;
  toUtc: string | null;
}

export interface AnalyticsRange {
  earliestSubmissionAt: string | null;
  latestSubmissionAt: string | null;
}

export interface AnalyticsSummary {
  submissions: number;
  uniqueContacts: number;
  repeatSubmissions: number;
}

export interface AnalyticsTrendPoint {
  day: string;
  submissions: number;
  uniqueContacts: number;
  repeatSubmissions: number;
}

export interface AnalyticsStatusPoint {
  status: LeadStatus;
  contacts: number;
}

export interface AnalyticsBreakdownPoint {
  key: string;
  submissions: number;
  uniqueContacts: number;
}

export interface AnalyticsResponse {
  range: AnalyticsRange;
  summary: AnalyticsSummary;
  trend: AnalyticsTrendPoint[];
  currentStatusFunnel: AnalyticsStatusPoint[];
  countryBreakdown: AnalyticsBreakdownPoint[];
  platformBreakdown: AnalyticsBreakdownPoint[];
  productBreakdown: AnalyticsBreakdownPoint[];
}
