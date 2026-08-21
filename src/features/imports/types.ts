export type SourceFormat = "XLSX" | "CSV";

export type ProductCode =
  | "FUE_MICROMOTOR_SYSTEMS"
  | "LONG_HAIR_FUE_SOLUTIONS"
  | "FUE_PUNCHES"
  | "IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS"
  | "MEDICAL_CHAIRS_CLINIC_FURNITURE"
  | "OTHER_GENERAL_INFORMATION"
  | "UNKNOWN";

export type NormalizationWarning =
  | "INVALID_EMAIL"
  | "INVALID_PHONE"
  | "INVALID_COUNTRY"
  | "INVALID_TIMESTAMP"
  | "MISSING_CONTACT_METHOD"
  | "UNKNOWN_PRODUCT";

export type IdentityDecision =
  | { outcome: "NEW_CONTACT" }
  | {
      outcome: "REPEAT_CONTACT";
      contact_id: string;
      matched_by: Array<"EMAIL" | "PHONE">;
    }
  | {
      outcome: "EXACT_DUPLICATE_SUBMISSION";
      external_lead_id: string;
    }
  | {
      outcome: "IDENTITY_CONFLICT_REVIEW";
      candidate_contact_ids: string[];
    }
  | { outcome: "ROW_ERROR"; code: string };

export interface NormalizedSubmission {
  rowNumber: number;
  externalLeadId: string;
  createdAtUtc: string | null;
  normalizedEmail: string | null;
  normalizedPhone: string | null;
  countryCode: string | null;
  productInterests: ProductCode[];
  warnings: NormalizationWarning[];
}

export interface ImportPreviewRow {
  rowNumber: number;
  fullName: string;
  rawEmail: string;
  rawPhone: string;
  rawCountry: string;
  rawProductAnswer: string;
  normalized: NormalizedSubmission;
  decision: IdentityDecision;
}

export interface ImportPreviewSource {
  fileName: string;
  fileSize: number | null;
  format: SourceFormat;
  sheetName: string | null;
  columnCount: number;
  ignoredAgencyColumns: string[];
}

export interface ImportPreviewSummary {
  totalRows: number;
  importableSubmissions: number;
  newContacts: number;
  repeatSubmissions: number;
  exactDuplicates: number;
  identityConflicts: number;
  rowErrors: number;
  warningCount: number;
}

export interface ImportPreview {
  source: ImportPreviewSource;
  summary: ImportPreviewSummary;
  rows: ImportPreviewRow[];
}

export interface CommitImportResult {
  batchId: string;
  summary: ImportPreviewSummary;
}

export interface ImportHistoryItem {
  batchId: string;
  fileName: string;
  format: SourceFormat | "UNKNOWN";
  sheetName: string;
  completedAt: string | null;
  status: string;
  totalRows: number;
  importedSubmissions: number;
  exactDuplicates: number;
  repeatSubmissions: number;
  warningCount: number;
  errorCount: number;
  appVersion: string;
}

export interface CommandError {
  code: string;
  message: string;
}
