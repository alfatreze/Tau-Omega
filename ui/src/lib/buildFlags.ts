/**
 * Build-time feature flags controlled via Vite env vars (`VITE_*`).
 *
 * - `VITE_LEGACY_TOOLS=0` hides the older/unfinished items under **Tools & settings**
 *   (`Compare cores`, `Backup`, `Packages`, `Problems`) from the sidebar while keeping
 *   `Send feedback`, `Settings`, and (when a Tau core is on the card) `Send diagnostics`
 *   (and keeping `Packages` reachable programmatically from the Tau update banner).
 * - Defaults to `true` when unset so local dev and existing CI behaviour suites remain unchanged.
 */
export const SHOW_LEGACY_TOOLS: boolean =
  (import.meta.env?.VITE_LEGACY_TOOLS ?? '1') !== '0';
