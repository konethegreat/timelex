// Written by Kone Tshivhinda & Claude
import type { Health, Meta } from "./types.js";

export interface ClientOptions {
  /** Base URL of the gateway, e.g. http://127.0.0.1:8088 */
  baseUrl: string;
  /** API key (sent as a Bearer token once the gateway enforces auth in Phase 1). */
  apiKey?: string;
  /** Override fetch (tests). */
  fetch?: typeof fetch;
}

export class TimeLexError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "TimeLexError";
  }
}

export class TimeLexClient {
  private readonly baseUrl: string;
  private readonly apiKey?: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: ClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, "");
    this.apiKey = options.apiKey;
    this.fetchImpl = options.fetch ?? fetch;
  }

  private async get<T>(path: string): Promise<T> {
    const headers: Record<string, string> = { accept: "application/json" };
    if (this.apiKey) headers.authorization = `Bearer ${this.apiKey}`;
    const response = await this.fetchImpl(`${this.baseUrl}${path}`, { headers });
    if (!response.ok) {
      throw new TimeLexError(`GET ${path} failed with ${response.status}`, response.status);
    }
    return (await response.json()) as T;
  }

  health(): Promise<Health> {
    return this.get<Health>("/health");
  }

  meta(): Promise<Meta> {
    return this.get<Meta>("/v1/meta");
  }
}
