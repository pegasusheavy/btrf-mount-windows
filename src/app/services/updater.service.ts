import { Injectable, signal } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface UpdateInfo {
  version: string;
  current_version: string;
  body: string | null;
  date: string | null;
}

@Injectable({
  providedIn: 'root'
})
export class UpdaterService {
  /** Whether an update is available */
  readonly updateAvailable = signal<boolean>(false);
  
  /** Information about the available update */
  readonly updateInfo = signal<UpdateInfo | null>(null);
  
  /** Whether currently checking for updates */
  readonly checking = signal<boolean>(false);
  
  /** Whether currently installing an update */
  readonly installing = signal<boolean>(false);
  
  /** Error message if update check/install failed */
  readonly error = signal<string | null>(null);

  constructor() {
    this.listenForUpdates();
  }

  /** Listen for update events from the backend */
  private async listenForUpdates(): Promise<void> {
    try {
      await listen<UpdateInfo>('update-available', (event) => {
        console.log('Update available:', event.payload);
        this.updateInfo.set(event.payload);
        this.updateAvailable.set(true);
      });
    } catch (e) {
      console.warn('Failed to listen for update events:', e);
    }
  }

  /** Manually check for updates */
  async checkForUpdates(): Promise<UpdateInfo | null> {
    this.checking.set(true);
    this.error.set(null);
    
    try {
      const update = await invoke<UpdateInfo | null>('check_update');
      
      if (update) {
        this.updateInfo.set(update);
        this.updateAvailable.set(true);
      } else {
        this.updateAvailable.set(false);
        this.updateInfo.set(null);
      }
      
      return update;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      this.error.set(errorMsg);
      console.error('Failed to check for updates:', e);
      return null;
    } finally {
      this.checking.set(false);
    }
  }

  /** Install the available update */
  async installUpdate(): Promise<void> {
    if (!this.updateAvailable()) {
      return;
    }
    
    this.installing.set(true);
    this.error.set(null);
    
    try {
      await invoke('install_update');
      // App will restart automatically after install
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      this.error.set(errorMsg);
      console.error('Failed to install update:', e);
    } finally {
      this.installing.set(false);
    }
  }

  /** Dismiss the update notification */
  dismissUpdate(): void {
    this.updateAvailable.set(false);
  }
}
