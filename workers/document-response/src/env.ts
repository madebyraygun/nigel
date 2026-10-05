export interface StoredObject {
  json<T>(): Promise<T>;
}

export interface PutOptions {
  onlyIf?: { etagDoesNotMatch?: string };
  httpMetadata?: { contentType?: string };
}

export interface PrivateBucket {
  get(key: string): Promise<StoredObject | null>;
  put(key: string, value: string, options?: PutOptions): Promise<unknown | null>;
}

export interface Limiter {
  limit(options: { key: string }): Promise<{ success: boolean }>;
}

export interface Env {
  PRIVATE: PrivateBucket;
  RATE_LIMITER: Limiter;
}
