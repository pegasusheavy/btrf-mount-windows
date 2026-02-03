import { Injectable, signal } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';

export interface DeviceInfo {
  path: string;
  size: number;
  sector_size: number;
  model: string | null;
  is_btrfs: boolean;
}

export interface VolumeInfo {
  uuid: string;
  label: string;
  total_bytes: number;
  bytes_used: number;
  num_devices: number;
  generation: number;
}

export interface SubvolumeInfo {
  id: number;
  parent_id: number;
  name: string;
  path: string;
  generation: number;
  flags: number;
}

export interface MountInfo {
  source: string;
  mount_point: string;
  read_only: boolean;
}

export interface MountRequest {
  source: string;
  drive_letter: string;
  read_only: boolean;
  subvolume_id: number | null;
}

@Injectable({
  providedIn: 'root',
})
export class TauriService {
  readonly isLoading = signal(false);
  readonly error = signal<string | null>(null);

  async listDevices(): Promise<DeviceInfo[]> {
    this.isLoading.set(true);
    this.error.set(null);
    try {
      return await invoke<DeviceInfo[]>('list_devices');
    } catch (e) {
      this.error.set(String(e));
      throw e;
    } finally {
      this.isLoading.set(false);
    }
  }

  async detectBtrfs(path: string): Promise<boolean> {
    try {
      return await invoke<boolean>('detect_btrfs', { path });
    } catch (e) {
      console.error('Failed to detect BTRFS:', e);
      return false;
    }
  }

  async mountVolume(request: MountRequest): Promise<MountInfo> {
    this.isLoading.set(true);
    this.error.set(null);
    try {
      return await invoke<MountInfo>('mount_volume', { request });
    } catch (e) {
      this.error.set(String(e));
      throw e;
    } finally {
      this.isLoading.set(false);
    }
  }

  async unmountVolume(mountPoint: string): Promise<void> {
    this.isLoading.set(true);
    this.error.set(null);
    try {
      await invoke('unmount_volume', { mountPoint });
    } catch (e) {
      this.error.set(String(e));
      throw e;
    } finally {
      this.isLoading.set(false);
    }
  }

  async listSubvolumes(source: string): Promise<SubvolumeInfo[]> {
    this.isLoading.set(true);
    this.error.set(null);
    try {
      return await invoke<SubvolumeInfo[]>('list_subvolumes', { source });
    } catch (e) {
      this.error.set(String(e));
      throw e;
    } finally {
      this.isLoading.set(false);
    }
  }

  async getVolumeInfo(source: string): Promise<VolumeInfo> {
    this.isLoading.set(true);
    this.error.set(null);
    try {
      return await invoke<VolumeInfo>('get_volume_info', { source });
    } catch (e) {
      this.error.set(String(e));
      throw e;
    } finally {
      this.isLoading.set(false);
    }
  }

  async listMounts(): Promise<MountInfo[]> {
    try {
      return await invoke<MountInfo[]>('list_mounts');
    } catch (e) {
      console.error('Failed to list mounts:', e);
      return [];
    }
  }

  formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }
}
