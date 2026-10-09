import type { Limiter, PrivateBucket, PutOptions, StoredObject } from '../src/env.js';

export class MemoryBucket implements PrivateBucket {
  objects = new Map<string, string>();

  async get(key: string): Promise<StoredObject | null> {
    const value = this.objects.get(key);
    if (value === undefined) return null;
    return { json: async <T>() => JSON.parse(value) as T };
  }

  async put(key: string, value: string, options?: PutOptions): Promise<object | null> {
    if (options?.onlyIf?.etagDoesNotMatch === '*' && this.objects.has(key)) return null;
    this.objects.set(key, value);
    return {};
  }
}

export class CountingLimiter implements Limiter {
  private readonly counts = new Map<string, number>();

  constructor(private readonly max: number) {}

  async limit({ key }: { key: string }): Promise<{ success: boolean }> {
    const count = (this.counts.get(key) ?? 0) + 1;
    this.counts.set(key, count);
    return { success: count <= this.max };
  }
}
