export interface SentinelConfig {
  escalationThreshold: number;
  minimumRemainingVoteMinutes: number;
  pollIntervalMs: number;
}

function readNumber(env: NodeJS.ProcessEnv, key: string, fallback: number, min: number, max: number): number {
  const raw = env[key];
  if (raw === undefined || raw === "") return fallback;
  const value = Number(raw);
  if (!Number.isFinite(value) || value < min || value > max) {
    throw new Error(`${key} must be a number between ${min} and ${max}.`);
  }
  return value;
}

/** Defaults favor a conservative review window and avoid fast polling. */
export function loadSentinelConfig(env: NodeJS.ProcessEnv = process.env): SentinelConfig {
  return {
    escalationThreshold: readNumber(env, "SENTINEL_ESCALATION_THRESHOLD", 0.85, 0.5, 1),
    minimumRemainingVoteMinutes: readNumber(env, "SENTINEL_MIN_REMAINING_VOTE_MINUTES", 60, 0, 10_080),
    pollIntervalMs: readNumber(env, "SENTINEL_POLL_INTERVAL_MS", 900_000, 60_000, 86_400_000),
  };
}
