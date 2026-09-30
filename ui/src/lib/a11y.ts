/** Shared dialog behaviour, used by every modal in the app so they all behave
 * the same for keyboard and screen-reader users:
 *  - focus moves in when the dialog opens (to the element marked
 *    `data-autofocus`, else the first text field, else the primary button),
 *  - Tab and Shift+Tab stay inside it,
 *  - Escape is handled by the caller (it knows whether closing is allowed),
 *  - focus returns to where it was when the dialog closes (if that control
 *    still exists). */
export function modal(node: HTMLElement) {
  const previous = document.activeElement as HTMLElement | null;
  const focusable = () => [...node.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), [href], select, textarea, [tabindex]:not([tabindex="-1"])')];
  const first = node.querySelector<HTMLElement>('[data-autofocus]')
    ?? node.querySelector<HTMLElement>('input:not([type=checkbox]):not([type=radio])')
    ?? node.querySelector<HTMLElement>('button.primary')
    ?? focusable()[0];
  first?.focus();
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== 'Tab') return;
    const items = focusable();
    if (!items.length) return;
    const head = items[0], tail = items[items.length - 1];
    if (e.shiftKey && document.activeElement === head) { e.preventDefault(); tail.focus(); }
    else if (!e.shiftKey && document.activeElement === tail) { e.preventDefault(); head.focus(); }
  };
  node.addEventListener('keydown', onKey);
  return { destroy() { node.removeEventListener('keydown', onKey); if (previous?.isConnected) previous.focus(); } };
}

/** Keeps Tab inside `node` while `active` is true (a slide-in menu over a backdrop). */
export function trap(node: HTMLElement, active: boolean) {
  let on = active;
  const focusable = () => [...node.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), [href], select, textarea, [tabindex]:not([tabindex="-1"])')].filter((el) => el.offsetParent !== null);
  const onKey = (e: KeyboardEvent) => {
    if (!on || e.key !== 'Tab') return;
    const items = focusable();
    if (!items.length) return;
    const head = items[0], tail = items[items.length - 1];
    if (e.shiftKey && document.activeElement === head) { e.preventDefault(); tail.focus(); }
    else if (!e.shiftKey && document.activeElement === tail) { e.preventDefault(); head.focus(); }
  };
  node.addEventListener('keydown', onKey);
  return { update(next: boolean) { on = next; }, destroy() { node.removeEventListener('keydown', onKey); } };
}
