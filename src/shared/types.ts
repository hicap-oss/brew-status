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
