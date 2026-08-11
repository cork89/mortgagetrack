export type TabId =
  | "summary"
  | "calendar"
  | "payments"
  | "improvements"
  | "chart";

export type ChartGrain = "monthly" | "yearly";
export type LoanFormMode = "create" | "edit";
export type ImprovementPopoverMode = "add" | "edit";
export type ProfileManagerMode = "create" | "edit";

export interface ProfileOption {
  id: string;
  name: string;
  is_shared: boolean;
  principal: number;
  rate: number;
  term_years: number;
  start_date: string;
  auto_mark_due_paid: boolean;
}

export interface CopyProfileData {
  profile: ProfileOption;
  can_create_profile: boolean;
}

export interface DeleteProfileData {
  deleted_id: string;
  active_id: string | null;
  can_create_profile: boolean;
  profiles: ProfileOption[];
}

export interface ApiOkResponse<T> {
  ok: true;
  data: T;
}

export interface ApiErrResponse {
  ok?: false;
  error?: string;
}

export type ApiResponse<T> = ApiOkResponse<T> | ApiErrResponse;

export interface ChartBucket {
  label: string;
  year: number;
  principal: number;
  interest: number;
  payment: number;
  count?: number;
}

export interface MarkPanelsStaleDetail {
  keep?: TabId;
  invalidateChart?: boolean;
}

export interface ActivateTabOptions {
  focus?: boolean;
  syncUrl?: boolean;
}

export interface OpenProfileManagerOptions {
  mode?: ProfileManagerMode;
  profileId?: string;
}

export interface TabActivationHooks {
  onPaymentsActivated?: () => void;
  onChartActivated?: () => void;
}
