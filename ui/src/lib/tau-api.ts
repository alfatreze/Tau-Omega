import { invoke } from './backend';
import type { Comparison, Core, DuplicateGroup, LibrarySummary, MediaScan, Plan, Setting, SyncReport } from './types';

export const inspectCard = (path: string) => invoke<Core[]>('inspect_card', { path });
export const readPersistedSettings = (path: string) => invoke<Setting[]>('read_persisted_settings', { path });
export const planSync = (sources: string[], destination: string, embedCovers: boolean) => invoke<Plan>('plan_sync', { sources, destination, embedCovers });
export const executeSync = (sources: string[], destination: string, confirmation: string, manifestPath: string, embedCovers: boolean, jobId: string) => invoke<SyncReport>('execute_sync', { sources, destination, confirmation, manifestPath, embedCovers, jobId });
export const compareMedia = (left: string, right: string) => invoke<Comparison>('compare_media', { left, right });
export const planCoreCopy = (source: string, destination: string) => invoke<Plan>('plan_core_copy', { source, destination });
export const executeCoreCopy = (source: string, destination: string, confirmation: string, manifestPath: string, jobId: string) => invoke<SyncReport>('execute_core_copy', { source, destination, confirmation, manifestPath, jobId });
export const executeCoreMove = (source: string, destination: string, confirmation: string, backupPath: string, manifestPath: string, jobId: string) => invoke<SyncReport>('execute_core_move', { source, destination, confirmation, deleteConfirmation: confirmation, backupPath, manifestPath, jobId });
export const scanMedia = (path: string, jobId: string) => invoke<MediaScan>('scan_media', { path, jobId });
export const exportPlaylist = (mediaRoot: string, playlistName: string, output: string) => invoke<void>('export_playlist', { mediaRoot, playlistName, output });
export const findDuplicates = (path: string) => invoke<DuplicateGroup[]>('find_duplicates', { path });
export const readJournal = (path: string) => invoke<unknown>('read_journal', { path });
export const summarizeLibrary = (path: string, jobId: string) => invoke<LibrarySummary>('summarize_library', { path, jobId });
export { cancelJob, newJobId, onProgress, errorMessage } from './backend';
