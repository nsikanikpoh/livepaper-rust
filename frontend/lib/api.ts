/**
 * lib/api.ts
 *
 * Central API client for LivePaper.
 * - Fetches a fresh Clerk session token before every request
 * - Attaches it as `Authorization: Bearer <token>`
 * - Throws a typed ApiError on non-2xx responses
 *
 * Usage:
 *   import { apiClient } from '@/lib/api';
 *   const papers = await apiClient.get('/papers');
 *   const result = await apiClient.post('/chat', { message, session_id });
 */

import { useAuth } from '@clerk/nextjs';

const BASE_URL = (process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080').replace(/\/$/, '');

// ── Error type ────────────────────────────────────────────────────────────────

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
    public readonly body?: unknown,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

// ── Token getter — injected at call site ──────────────────────────────────────

type TokenGetter = () => Promise<string | null>;

// ── Core fetch wrapper ────────────────────────────────────────────────────────

async function apiFetch<T>(
  getToken: TokenGetter,
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const token = await getToken();

  const headers = new Headers(init.headers);
  headers.set('Content-Type', 'application/json');
  if (token) {
    headers.set('Authorization', `Bearer ${token}`);
  }

  const res = await fetch(`${BASE_URL}${path}`, { ...init, headers });

  if (!res.ok) {
    let body: unknown;
    try { body = await res.json(); } catch { body = await res.text(); }
    const message =
      (typeof body === 'object' && body !== null && 'error' in body)
        ? (body as { error: string }).error
        : `HTTP ${res.status}`;
    throw new ApiError(res.status, message, body);
  }

  // 204 No Content
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

// ── Factory — call once inside a component/hook that has access to useAuth ────

export function createApiClient(getToken: TokenGetter) {
  return {
    get<T>(path: string): Promise<T> {
      return apiFetch<T>(getToken, path, { method: 'GET' });
    },

    post<T>(path: string, body?: unknown): Promise<T> {
      return apiFetch<T>(getToken, path, {
        method: 'POST',
        body: body !== undefined ? JSON.stringify(body) : undefined,
      });
    },

    put<T>(path: string, body?: unknown): Promise<T> {
      return apiFetch<T>(getToken, path, {
        method: 'PUT',
        body: body !== undefined ? JSON.stringify(body) : undefined,
      });
    },

    delete<T>(path: string): Promise<T> {
      return apiFetch<T>(getToken, path, { method: 'DELETE' });
    },
  };
}

// ── Hook — use inside React components ───────────────────────────────────────

/**
 * Returns a ready-to-use API client that will attach the Clerk JWT
 * on every call. Use this inside any component or hook.
 *
 * const api = useApiClient();
 * const papers = await api.get<Paper[]>('/papers');
 */
export function useApiClient() {
  const { getToken } = useAuth();
  const tokenGetter: TokenGetter = () => getToken();
  return createApiClient(tokenGetter);
}
