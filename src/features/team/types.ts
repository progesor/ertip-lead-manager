export type StaffRole = "ADMIN" | "MANAGER" | "SALES";

export interface StaffMember {
  id: string;
  displayName: string;
  email: string | null;
  role: StaffRole;
  isActive: boolean;
  authSubject: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface StaffMemberInput {
  displayName: string;
  email: string | null;
  role: StaffRole;
}

export interface LeadAssignee {
  id: string;
  displayName: string;
  isActive: boolean;
}

export interface AssignmentResult {
  changed: boolean;
  assignee: LeadAssignee | null;
}
