/** Files dragged in from the computer, in the desktop app and in the browser mock.
 *
 * In the app Tauri delivers the real paths (`onDragDropEvent`, with physical coordinates); in a plain browser (the dev mock, the browser checks)
 * the DOM events give File objects without paths, so the mock uses `/mock/<name>`. Either way the handler gets `over` (with the point, so a
 * drop zone can light up), `leave`, and `drop` (with the paths), and finds the zone under the point with `zoneAt`. */
export type DropEvent =
  | { type: 'over'; x: number; y: number }
  | { type: 'leave' }
  | { type: 'drop'; x: number; y: number; paths: string[] };

declare global { interface Window { __TAURI_INTERNALS__?: unknown } }

export function onFileDrop(handler: (e: DropEvent) => void): () => void {
  let off: (() => void) | null = null; let cancelled = false;
  if (typeof window !== 'undefined' && window.__TAURI_INTERNALS__) {
    import('@tauri-apps/api/webview').then(async ({ getCurrentWebview }) => {
      const un = await getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        const dpr = window.devicePixelRatio || 1;
        if (p.type === 'over' || p.type === 'enter') handler({ type: 'over', x: p.position.x / dpr, y: p.position.y / dpr });
        else if (p.type === 'drop') handler({ type: 'drop', x: p.position.x / dpr, y: p.position.y / dpr, paths: p.paths });
        else handler({ type: 'leave' });
      });
      if (cancelled) un(); else off = un;
    }).catch(() => { /* no drag and drop: the Add files button still works */ });
  } else {
    const over = (e: DragEvent) => { if (e.dataTransfer?.types.includes('Files')) { e.preventDefault(); handler({ type: 'over', x: e.clientX, y: e.clientY }); } };
    const drop = (e: DragEvent) => {
      if (!e.dataTransfer?.files.length) return;
      e.preventDefault();
      handler({ type: 'drop', x: e.clientX, y: e.clientY, paths: [...e.dataTransfer.files].map((f) => `/mock/${f.name}`) });
    };
    const leave = (e: DragEvent) => { if (!e.relatedTarget) handler({ type: 'leave' }); };
    window.addEventListener('dragover', over); window.addEventListener('drop', drop); window.addEventListener('dragleave', leave);
    off = () => { window.removeEventListener('dragover', over); window.removeEventListener('drop', drop); window.removeEventListener('dragleave', leave); };
  }
  return () => { cancelled = true; off?.(); };
}

/** The `data-drop-zone` value of the element under a point, or '' when there is none. */
export function zoneAt(x: number, y: number): string {
  for (const el of document.elementsFromPoint(x, y)) {
    const zone = (el as HTMLElement).dataset?.dropZone;
    if (zone) return zone;
  }
  return '';
}
