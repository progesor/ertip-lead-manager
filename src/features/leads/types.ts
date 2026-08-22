export type LeadStatus =
  | "NEW"
  | "CONTACTED"
  | "REPLIED"
  | "QUALIFIED"
  | "QUOTE_SENT"
  | "WON"
  | "LOST"
  | "INVALID";

export type ProductCode =
  | "FUE_MICROMOTOR_SYSTEMS"
  | "LONG_HAIR_FUE_SOLUTIONS"
  | "FUE_PUNCHES"
  | "IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS"
  | "MEDICAL_CHAIRS_CLINIC_FURNITURE"
  | "OTHER_GENERAL_INFORMATION"
  | "UNKNOWN";

export type DataQualityIssueType =
  | "INVALID_EMAIL"
  | "INVALID_PHONE"
  | "INVALID_COUNTRY"
  | "INVALID_TIMESTAMP"
  | "MISSING_CONTACT_METHOD"
  | "UNKNOWN_PRODUCT";

export type LeadListSort = "LATEST_DESC" | "LATEST_ASC" | "NAME_ASC" | "NAME_DESC";

export interface LeadListRequest {
  search: string | null;
  status: LeadStatus | null;
  countryCode: string | null;
  productCode: ProductCode | null;
  repeatOnly: boolean;
  warningOnly: boolean;
  sort: LeadListSort;
  page: number;
  pageSize: number;
}

export interface LeadWarningSummary {
  issueType: DataQualityIssueType;
  count: number;
}

export interface LeadListItem {
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
  warningSummaries: LeadWarningSummary[];
}

export interface LeadListResponse {
  items: LeadListItem[];
  total: number;
  page: number;
  pageSize: number;
  totalPages: number;
}

export interface LeadFilterOptions {
  countries: string[];
}

export interface LeadDetailContact {
  id: string;
  displayName: string;
  primaryEmail: string | null;
  primaryPhone: string | null;
  countryCode: string | null;
  status: LeadStatus;
  createdAt: string;
  updatedAt: string;
  latestSubmissionAt: string | null;
  submissionCount: number;
  productInterests: ProductCode[];
}

export interface LeadDetailSubmission {
  id: string;
  externalLeadId: string;
  sourceCreatedAtUtc: string | null;
  sourceCreatedAtRaw: string;
  adId: string | null;
  adName: string | null;
  adsetId: string | null;
  adsetName: string | null;
  campaignId: string | null;
  campaignName: string | null;
  formId: string | null;
  formName: string | null;
  isOrganic: boolean | null;
  platform: string | null;
  rawProcedureAnswer: string | null;
  rawProductAnswer: string | null;
  rawFullName: string | null;
  rawEmail: string | null;
  rawPhone: string | null;
  rawCountry: string | null;
  rawLeadStatus: string | null;
  rawPayloadJson: string;
  productInterests: ProductCode[];
}

export interface LeadDetailQualityIssue {
  id: string;
  leadSubmissionId: string | null;
  issueType: DataQualityIssueType;
  severity: string;
  detailsJson: string;
  status: string;
  createdAt: string;
  resolvedAt: string | null;
}

export interface LeadDetailActivity {
  id: string;
  activityType: string;
  occurredAt: string;
  payloadJson: string;
}

export interface LeadDetailResponse {
  contact: LeadDetailContact;
  submissions: LeadDetailSubmission[];
  qualityIssues: LeadDetailQualityIssue[];
  activities: LeadDetailActivity[];
}

export interface CommandError {
  code: string;
  message: string;
}
