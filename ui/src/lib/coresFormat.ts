export const size = (b: number) => (b < 1024 ? `${b} B` : b < 1048576 ? `${(b / 1024).toFixed(0)} KB` : b < 1073741824 ? `${(b / 1048576).toFixed(1)} MB` : `${(b / 1073741824).toFixed(2)} GB`);
export const day = (unix: number) => (unix ? new Date(unix * 1000).toISOString().slice(0, 10) : '');
export const KIND_LABEL: Record<string, string> = { Rom: 'ROM', Bios: 'BIOS', Tape: 'Tape', Disk: 'Disk', Save: 'Save', State: 'Save state', Other: 'File' };
export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
