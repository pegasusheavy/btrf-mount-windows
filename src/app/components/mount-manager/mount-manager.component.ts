import { Component, OnInit, signal, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TauriService, MountInfo, DeviceInfo } from '../../services/tauri.service';

@Component({
  selector: 'app-mount-manager',
  standalone: true,
  imports: [CommonModule, FormsModule],
  template: `
    <div class="space-y-6">
      <div class="flex items-center justify-between">
        <h2 class="text-2xl font-bold text-gray-900 dark:text-white">Mount Manager</h2>
        <button
          (click)="refreshMounts()"
          class="px-4 py-2 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
        >
          Refresh
        </button>
      </div>

      <!-- Mount New Volume -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Mount New Volume</h3>
        
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Source (Image file or physical drive)
            </label>
            <div class="flex gap-2">
              <input
                type="text"
                [(ngModel)]="sourcePath"
                placeholder="C:\\path\\to\\image.img or \\\\.\\PhysicalDrive1"
                class="flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
              <button
                (click)="browseFile()"
                class="px-4 py-2 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600"
              >
                Browse
              </button>
            </div>
          </div>

          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Drive Letter
            </label>
            <select
              [(ngModel)]="driveLetter"
              class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
            >
              @for (letter of availableLetters; track letter) {
                <option [value]="letter">{{ letter }}:</option>
              }
            </select>
          </div>
        </div>

        <div class="mt-4 flex items-center gap-6">
          <label class="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              [(ngModel)]="readOnly"
              class="w-4 h-4 text-blue-500 border-gray-300 rounded focus:ring-blue-500"
            />
            <span class="text-sm text-gray-700 dark:text-gray-300">Read-only</span>
          </label>
        </div>

        <div class="mt-6">
          <button
            (click)="mount()"
            [disabled]="!sourcePath || tauri.isLoading()"
            class="px-6 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            @if (tauri.isLoading()) {
              <span class="flex items-center gap-2">
                <span class="spinner w-4 h-4"></span>
                Mounting...
              </span>
            } @else {
              Mount Volume
            }
          </button>
        </div>

        @if (tauri.error()) {
          <div class="mt-4 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
            <p class="text-sm text-red-700 dark:text-red-400">{{ tauri.error() }}</p>
          </div>
        }
      </div>

      <!-- Active Mounts -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Active Mounts</h3>
        
        @if (mounts().length === 0) {
          <p class="text-gray-500 dark:text-gray-400 text-center py-8">No active mounts</p>
        } @else {
          <div class="space-y-3">
            @for (mount of mounts(); track mount.mount_point) {
              <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
                <div>
                  <div class="font-medium text-gray-900 dark:text-white">{{ mount.mount_point }}</div>
                  <div class="text-sm text-gray-500 dark:text-gray-400">{{ mount.source || 'Unknown source' }}</div>
                  @if (mount.read_only) {
                    <span class="inline-block mt-1 px-2 py-0.5 text-xs bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-400 rounded">
                      Read-only
                    </span>
                  }
                </div>
                <button
                  (click)="unmount(mount.mount_point)"
                  class="px-4 py-2 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
                >
                  Unmount
                </button>
              </div>
            }
          </div>
        }
      </div>
    </div>
  `,
})
export class MountManagerComponent implements OnInit {
  readonly tauri = inject(TauriService);

  sourcePath = '';
  driveLetter = 'Z';
  readOnly = false;

  mounts = signal<MountInfo[]>([]);
  availableLetters = ['D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z'];

  ngOnInit() {
    this.refreshMounts();
  }

  async refreshMounts() {
    const mounts = await this.tauri.listMounts();
    this.mounts.set(mounts);
  }

  browseFile() {
    // TODO: Use Tauri file dialog
    console.log('Browse file - implement with Tauri dialog');
  }

  async mount() {
    try {
      await this.tauri.mountVolume({
        source: this.sourcePath,
        drive_letter: this.driveLetter,
        read_only: this.readOnly,
        subvolume_id: null,
      });
      await this.refreshMounts();
      this.sourcePath = '';
    } catch (e) {
      console.error('Mount failed:', e);
    }
  }

  async unmount(mountPoint: string) {
    try {
      await this.tauri.unmountVolume(mountPoint);
      await this.refreshMounts();
    } catch (e) {
      console.error('Unmount failed:', e);
    }
  }
}
