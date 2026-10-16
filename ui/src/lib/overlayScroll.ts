// Scroll bars that stay out of the way. Native vertical bars are hidden (styles.css); this draws one slim thumb over the area being scrolled
// or pointed at. It only exists where the content overflows, it fades after a moment, and its length is capped (MAX_THUMB), so a short overflow
// never produces a giant bar. The thumb can be dragged like a normal one. Horizontal bars stay native.
const MAX_THUMB = 96;
const MIN_THUMB = 28;
const IDLE_MS = 1000;
const INSET = 3;

export function installOverlayScroll() {
  const thumb = document.createElement('div');
  thumb.className = 'os-thumb';
  thumb.setAttribute('aria-hidden', 'true');
  document.body.appendChild(thumb);
  let target: Element | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let raf = 0;
  let hovering = false;
  let lastPointer: EventTarget | null = null;
  let drag: { startY: number; startTop: number; track: number; range: number } | null = null;

  const scroller = (el: Element | null): Element | null => {
    for (let e = el; e; e = e.parentElement) {
      if (e === document.body || e === document.documentElement) return document.scrollingElement ?? document.documentElement;
      const o = getComputedStyle(e).overflowY;
      if ((o === 'auto' || o === 'scroll') && e.scrollHeight > e.clientHeight + 1) return e;
    }
    return document.scrollingElement ?? document.documentElement;
  };
  const isRoot = (e: Element) => e === document.scrollingElement || e === document.documentElement || e === document.body;
  const area = (e: Element) => (isRoot(e) ? { top: 0, height: window.innerHeight, right: window.innerWidth } : (() => { const r = e.getBoundingClientRect(); return { top: r.top, height: r.height, right: r.right }; })());

  function place() {
    raf = 0;
    if (!target || !target.isConnected) return hide();
    const { scrollTop, scrollHeight, clientHeight } = target;
    const overflow = scrollHeight - clientHeight;
    if (overflow <= 1) return hide();
    const a = area(target);
    const len = Math.max(MIN_THUMB, Math.min(MAX_THUMB, (clientHeight / scrollHeight) * a.height));
    const track = a.height - len - INSET * 2;
    const y = a.top + INSET + (scrollTop / overflow) * Math.max(0, track);
    thumb.style.height = `${len}px`;
    thumb.style.transform = `translate(${a.right - 7 - INSET}px, ${y}px)`;
    thumb.classList.add('on');
    raf = requestAnimationFrame(place);
  }
  function hide() {
    if (drag || hovering) return;
    thumb.classList.remove('on');
    if (raf) cancelAnimationFrame(raf);
    raf = 0; target = null;
  }
  function show(el: Element | null) {
    if (!el) return;
    if (el.scrollHeight - el.clientHeight <= 1) { if (target === el) hide(); return; }
    target = el;
    clearTimeout(timer);
    timer = setTimeout(hide, IDLE_MS);
    if (!raf) raf = requestAnimationFrame(place);
  }

  document.addEventListener('scroll', (e) => show(e.target instanceof Element ? e.target : document.scrollingElement), true);
  document.addEventListener('pointermove', (e) => {
    if (drag && target) {
      const delta = e.clientY - drag.startY;
      target.scrollTop = drag.startTop + (delta / Math.max(1, drag.track)) * drag.range;
      return;
    }
    if (e.target === thumb || e.target === lastPointer) return;
    lastPointer = e.target;
    const el = scroller(e.target instanceof Element ? e.target : null);
    if (el && el !== target) show(el); else if (el === target) { clearTimeout(timer); timer = setTimeout(hide, IDLE_MS); }
  }, true);
  window.addEventListener('resize', () => target && show(target));
  thumb.addEventListener('pointerenter', () => { hovering = true; clearTimeout(timer); });
  thumb.addEventListener('pointerleave', () => { hovering = false; timer = setTimeout(hide, IDLE_MS); });
  thumb.addEventListener('pointerdown', (e) => {
    if (!target) return;
    e.preventDefault();
    thumb.setPointerCapture(e.pointerId);
    const a = area(target);
    const len = thumb.getBoundingClientRect().height;
    drag = { startY: e.clientY, startTop: target.scrollTop, track: a.height - len - INSET * 2, range: target.scrollHeight - target.clientHeight };
    thumb.classList.add('drag');
  });
  const end = (e: PointerEvent) => { if (!drag) return; drag = null; thumb.classList.remove('drag'); try { thumb.releasePointerCapture(e.pointerId); } catch { /* released */ } timer = setTimeout(hide, IDLE_MS); };
  thumb.addEventListener('pointerup', end);
  thumb.addEventListener('pointercancel', end);
}
