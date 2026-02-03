import { Component, signal, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TauriService, SubvolumeInfo } from '../../services/tauri.service';

interface Snapshot extends SubvolumeInfo {
  created?: Date;
}

@Component({
  selector: 'app-snapshot-panel',
  standalone: true,
  imports: [CommonModule, FormsModule],
  template: `
    <div class="space-y-6">
      <div class="flex items-center justify-between">
        <h2 class="text-2xl font-bold text-gray-900 dark:text-white">Snapshots</h2>
      </div>

      <!-- Create Snapshot -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Create Snapshot</h3>
        
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Source Subvolume
            </label>
            <input
              type="text"
              [(ngModel)]="sourceSubvolume"
              placeholder="Enter subvolume path..."
              class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Snapshot Name
            </label>
            <input
              type="text"
              [(ngModel)]="snapshotName"
              placeholder="my-snapshot"
              class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
            />
          </div>
        </div>

        <div class="mt-4 flex items-center gap-4">
          <label class="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              [(ngModel)]="readOnlySnapshot"
              class="w-4 h-4 text-blue-500 border-gray-300 rounded focus:ring-blue-500"
            />
            <span class="text-sm text-gray-700 dark:text-gray-300">Read-only snapshot</span>
          </label>
        </div>

        <div class="mt-6">
          <button
            (click)="createSnapshot()"
            [disabled]="!sourceSubvolume || !snapshotName"
            class="px-6 py-2 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Create Snapshot
          </button>
        </div>
      </div>

      <!-- Snapshot List -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Existing Snapshots</h3>
        
        @if (snapshots().length === 0) {
          <p class="text-gray-500 dark:text-gray-400 text-center py-8">
            No snapshots found. Create a snapshot above.
          </p>
        } @else {
          <div class="space-y-3">
            @for (snapshot of snapshots(); track snapshot.id) {
              <div class="p-4 border border-gray-200 dark:border-gray-700 rounded-lg">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-3">
                    <svg class="w-5 h-5 text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/>
                    </svg>
                    <div>
                      <div class="font-medium text-gray-900 dark:text-white">{{ snapshot.name }}</div>
                      <div class="text-sm text-gray-500 dark:text-gray-400">
                        ID: {{ snapshot.id }} · Gen: {{ snapshot.generation }}
                      </div>
                    </div>
                  </div>
                  <div class="flex items-center gap-2">
                    @if (isReadOnly(snapshot)) {
                      <span class="px-2 py-1 text-xs bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-400 rounded">
                        Read-only
                      </span>
                    }
                    <button
                      (click)="deleteSnapshot(snapshot)"
                      class="p-2 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
                    >
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            }
          </div>
        }
      </div>
    </div>
  `,
})
export class SnapshotPanelComponent {
  readonly tauri = inject(TauriService);

  sourceSubvolume = '';
  snapshotName = '';
  readOnlySnapshot = true;

  snapshots = signal<Snapshot[]>([]);

  async createSnapshot() {
    // TODO: Implement snapshot creation via Tauri command
    console.log('Create snapshot:', this.snapshotName, 'from', this.sourceSubvolume);
    alert('Snapshot creation not yet implemented');
  }

  async deleteSnapshot(snapshot: Snapshot) {
    if (confirm(`Delete snapshot "${snapshot.name}"?`)) {
      // TODO: Implement snapshot deletion via Tauri command
      console.log('Delete snapshot:', snapshot.id);
      alert('Snapshot deletion not yet implemented');
    }
  }

  isReadOnly(snapshot: Snapshot): boolean {
    return (snapshot.flags & 1) !== 0;
  }
}
