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
  warningCount: number;
  warningTypes: DataQualityIssueType[];
}

export interface LeadListResponse {
  items: LeadListItem[];
  total: number;
  page: number;
  pageSize: number;
  totalPages: number;
}

export interface CommandError {
  code: string;
  message: string;
}
