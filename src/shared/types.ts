export interface StatsCache {
  version: number;
  lastComputedDate: string;
  dailyActivity: DailyActivity[];
  dailyModelTokens: DailyModelTokens[];
  modelUsage: Record<string, ModelUsage>;
  totalSessions: number;
  totalMessages: number;
  longestSession: LongestSession | null;
  firstSessionDate: string | null;
  hourCounts: Record<string, number>;
  totalSpeculationTimeSavedMs: number;
}

export interface DailyActivity {
  date: string;
  messageCount: number;
  sessionCount: number;
  toolCallCount: number;
}

export interface DailyModelTokens {
  date: string;
  tokensByModel: Record<string, number>;
}

export interface ModelUsage {
  inputTokens: number;
  outputTokens: number;
  cacheReadInputTokens: number;
  cacheCreationInputTokens: number;
  webSearchRequests: number;
  costUsd: number;
  contextWindow: number;
  maxOutputTokens: number;
}

export interface LongestSession {
  sessionId: string;
  duration: number;
  messageCount: number;
  timestamp: string;
}

export interface HistoryEntry {
  display: string;
  timestamp: number;
  project: string | null;
  sessionId: string | null;
}

export interface TodaySummary {
  date: string;
  totalTokens: number;
  tokensByModel: Record<string, number>;
  messages: number;
  sessions: number;
  toolCalls: number;
}

export interface DailyTokenTotal {
  date: string;
  total: number;
  byModel: Record<string, number>;
}

export interface ProfileResponse {
  account: AccountInfo;
  organization: OrganizationInfo;
}

export interface AccountInfo {
  uuid: string;
  full_name: string;
  display_name: string;
  email: string;
  has_claude_max: boolean;
  has_claude_pro: boolean;
  created_at: string;
}

export interface OrganizationInfo {
  uuid: string;
  name: string;
  organization_type: string;
  billing_type: string;
  rate_limit_tier: string;
  has_extra_usage_enabled: boolean;
  subscription_status: string;
  subscription_created_at: string;
}

export interface UsageLimits {
  five_hour: LimitEntry | null;
  seven_day: LimitEntry | null;
  seven_day_opus: LimitEntry | null;
  seven_day_sonnet: LimitEntry | null;
  seven_day_cowork: LimitEntry | null;
  seven_day_oauth_apps: LimitEntry | null;
  extra_usage: ExtraUsage | null;
}

export interface LimitEntry {
  utilization: number;
  resets_at: string | null;
}

export interface UpdateResult {
  updateAvailable: boolean;
  version: string | null;
}

export interface ExtraUsage {
  is_enabled: boolean;
  monthly_limit: number | null;
  used_credits: number | null;
  utilization: number | null;
}
