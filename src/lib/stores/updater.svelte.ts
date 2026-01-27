import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error';

interface UpdateState {
  status: UpdateStatus;
  update: Update | null;
  progress: number;
  error: string | null;
  version: string | null;
  releaseNotes: string | null;
}

function createUpdaterStore() {
  let state = $state<UpdateState>({
    status: 'idle',
    update: null,
    progress: 0,
    error: null,
    version: null,
    releaseNotes: null,
  });

  return {
    get status() {
      return state.status;
    },
    get update() {
      return state.update;
    },
    get progress() {
      return state.progress;
    },
    get error() {
      return state.error;
    },
    get version() {
      return state.version;
    },
    get releaseNotes() {
      return state.releaseNotes;
    },
    get isUpdateAvailable() {
      return state.status === 'available' || state.status === 'downloading' || state.status === 'ready';
    },

    async checkForUpdates(silent = false): Promise<boolean> {
      if (state.status === 'checking' || state.status === 'downloading') {
        return false;
      }

      state.status = 'checking';
      state.error = null;

      try {
        const update = await check();

        if (update) {
          state.update = update;
          state.status = 'available';
          state.version = update.version;
          state.releaseNotes = update.body ?? null;
          return true;
        } else {
          state.status = 'idle';
          state.update = null;
          return false;
        }
      } catch (err) {
        console.error('Update check failed:', err);
        state.status = 'error';
        state.error = err instanceof Error ? err.message : 'Update check failed';
        return false;
      }
    },

    async downloadAndInstall(): Promise<void> {
      if (!state.update) {
        throw new Error('No update available');
      }

      state.status = 'downloading';
      state.progress = 0;

      try {
        await state.update.downloadAndInstall((progress) => {
          if (progress.event === 'Started' && progress.data.contentLength) {
            state.progress = 0;
          } else if (progress.event === 'Progress') {
            // Calculate percentage based on downloaded chunks
            state.progress = Math.min(state.progress + 1, 99);
          } else if (progress.event === 'Finished') {
            state.progress = 100;
          }
        });

        state.status = 'ready';
      } catch (err) {
        console.error('Update download failed:', err);
        state.status = 'error';
        state.error = err instanceof Error ? err.message : 'Download failed';
        throw err;
      }
    },

    async restartApp(): Promise<void> {
      await relaunch();
    },

    dismiss(): void {
      state.status = 'idle';
      state.update = null;
      state.error = null;
      state.version = null;
      state.releaseNotes = null;
    },
  };
}

export const updater = createUpdaterStore();
