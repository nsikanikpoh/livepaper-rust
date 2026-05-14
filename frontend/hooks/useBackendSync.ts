/**
 * hooks/useBackendSync.ts
 *
 * Syncs the signed-in Clerk user to the Rust backend.
 * The backend's auth middleware already upserts the user on every
 * authenticated request, so a single lightweight GET is enough to
 * trigger the upsert immediately after sign-in — before the user
 * makes any real API call — ensuring the DB row exists.
 *
 * Call this once in your top-level authenticated layout/page.
 */

import { useEffect, useRef } from 'react';
import { useUser } from '@clerk/nextjs';
import { useApiClient } from '@/lib/api';

export function useBackendSync() {
  const { isLoaded, isSignedIn, user } = useUser();
  const api = useApiClient();
  const syncedRef = useRef<string | null>(null); // track last synced clerk id

  useEffect(() => {
    if (!isLoaded || !isSignedIn || !user) return;

    // Only sync once per session per user
    if (syncedRef.current === user.id) return;
    syncedRef.current = user.id;

    // A GET /papers is authenticated — the middleware will upsert the user.
    // We fire-and-forget; a failure here is non-fatal.
    api.get('/papers').catch(() => {
      // Silently ignore — the user will be synced on their next API call.
    });
  }, [isLoaded, isSignedIn, user?.id]);
}
