import { Component, OnInit, signal, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TauriService, SubvolumeInfo } from '../../services/tauri.service';

@Component({
  selector: 'app-subvolume-tree',
  standalone: true,
  imports: [CommonModule, FormsModule],
  template: `
    <div class="space-y-6">
      <div class="flex items-center justify-between">
        <h2 class="text-2xl font-bold text-gray-900 dark:text-white">Subvolumes</h2>
      </div>

      <!-- Source Selection -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
        <div class="flex gap-4">
          <input
            type="text"
            [(ngModel)]="sourcePath"
            placeholder="Enter BTRFS volume path..."
            class="flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
          />
          <button
            (click)="loadSubvolumes()"
            [disabled]="!sourcePath || tauri.isLoading()"
            class="px-6 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Load
          </button>
        </div>
      </div>

      <!-- Subvolume Tree -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Subvolume Tree</h3>
        
        @if (tauri.isLoading()) {
          <div class="flex items-center justify-center py-8">
            <span class="spinner w-8 h-8"></span>
          </div>
        } @else if (subvolumes().length === 0) {
          <p class="text-gray-500 dark:text-gray-400 text-center py-8">
            No subvolumes loaded. Enter a volume path above.
          </p>
        } @else {
          <div class="space-y-2">
            @for (subvol of subvolumes(); track subvol.id) {
              <div 
                class="p-4 border border-gray-200 dark:border-gray-700 rounded-lg hover:border-blue-300 dark:hover:border-blue-600 transition-colors"
                [style.margin-left.px]="getIndent(subvol)"
              >
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-3">
                    <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
                    </svg>
                    <div>
                      <div class="font-medium text-gray-900 dark:text-white">{{ subvol.name }}</div>
                      <div class="text-sm text-gray-500 dark:text-gray-400">
                        ID: {{ subvol.id }} · Path: {{ subvol.path }}
                      </div>
                    </div>
                  </div>
                  <div class="flex items-center gap-2">
                    @if (isReadOnly(subvol)) {
                      <span class="px-2 py-1 text-xs bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-400 rounded">
                        Read-only
                      </span>
                    }
                    <span class="text-sm text-gray-500 dark:text-gray-400">
                      Gen {{ subvol.generation }}
                    </span>
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
export class SubvolumeTreeComponent implements OnInit {
  readonly tauri = inject(TauriService);

  sourcePath = '';
  subvolumes = signal<SubvolumeInfo[]>([]);

  ngOnInit() {}

  async loadSubvolumes() {
    if (!this.sourcePath) return;
    
    try {
      const subvols = await this.tauri.listSubvolumes(this.sourcePath);
      this.subvolumes.set(subvols);
    } catch (e) {
      console.error('Failed to load subvolumes:', e);
    }
  }

  getIndent(subvol: SubvolumeInfo): number {
    // Calculate indent based on parent hierarchy
    // For now, simple flat display
    return subvol.parent_id === 0 ? 0 : 24;
  }

  isReadOnly(subvol: SubvolumeInfo): boolean {
    return (subvol.flags & 1) !== 0;
  }
}
