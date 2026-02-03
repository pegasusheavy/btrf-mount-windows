import { Component, OnInit, signal, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TauriService, DeviceInfo, VolumeInfo } from '../../services/tauri.service';

@Component({
  selector: 'app-device-browser',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="space-y-6">
      <div class="flex items-center justify-between">
        <h2 class="text-2xl font-bold text-gray-900 dark:text-white">Device Browser</h2>
        <button
          (click)="refreshDevices()"
          [disabled]="tauri.isLoading()"
          class="px-4 py-2 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors disabled:opacity-50"
        >
          @if (tauri.isLoading()) {
            <span class="flex items-center gap-2">
              <span class="spinner w-4 h-4"></span>
              Scanning...
            </span>
          } @else {
            Refresh
          }
        </button>
      </div>

      <!-- Physical Drives -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Physical Drives</h3>
        
        @if (devices().length === 0) {
          <p class="text-gray-500 dark:text-gray-400 text-center py-8">
            No physical drives found. Run as administrator for drive access.
          </p>
        } @else {
          <div class="space-y-3">
            @for (device of devices(); track device.path) {
              <div 
                class="p-4 border border-gray-200 dark:border-gray-700 rounded-lg hover:border-blue-300 dark:hover:border-blue-600 cursor-pointer transition-colors"
                (click)="selectDevice(device)"
                [class.border-blue-500]="selectedDevice()?.path === device.path"
                [class.bg-blue-50]="selectedDevice()?.path === device.path"
                [class.dark:bg-blue-900/20]="selectedDevice()?.path === device.path"
              >
                <div class="flex items-center justify-between">
                  <div>
                    <div class="font-medium text-gray-900 dark:text-white">{{ device.path }}</div>
                    <div class="text-sm text-gray-500 dark:text-gray-400">
                      {{ tauri.formatBytes(device.size) }} · {{ device.sector_size }} byte sectors
                    </div>
                    @if (device.model) {
                      <div class="text-sm text-gray-400 dark:text-gray-500">{{ device.model }}</div>
                    }
                  </div>
                  <div class="flex items-center gap-2">
                    @if (device.is_btrfs) {
                      <span class="px-3 py-1 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 text-sm rounded-full">
                        BTRFS
                      </span>
                    } @else {
                      <span class="px-3 py-1 bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 text-sm rounded-full">
                        Unknown
                      </span>
                    }
                  </div>
                </div>
              </div>
            }
          </div>
        }
      </div>

      <!-- Volume Info -->
      @if (selectedDevice() && volumeInfo()) {
        <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Volume Information</h3>
          
          <div class="grid grid-cols-2 gap-4">
            <div>
              <div class="text-sm text-gray-500 dark:text-gray-400">UUID</div>
              <div class="font-mono text-gray-900 dark:text-white">{{ volumeInfo()?.uuid }}</div>
            </div>
            <div>
              <div class="text-sm text-gray-500 dark:text-gray-400">Label</div>
              <div class="text-gray-900 dark:text-white">{{ volumeInfo()?.label || '(none)' }}</div>
            </div>
            <div>
              <div class="text-sm text-gray-500 dark:text-gray-400">Total Size</div>
              <div class="text-gray-900 dark:text-white">{{ tauri.formatBytes(volumeInfo()?.total_bytes || 0) }}</div>
            </div>
            <div>
              <div class="text-sm text-gray-500 dark:text-gray-400">Used</div>
              <div class="text-gray-900 dark:text-white">{{ tauri.formatBytes(volumeInfo()?.bytes_used || 0) }}</div>
            </div>
            <div>
              <div class="text-sm text-gray-500 dark:text-gray-400">Devices</div>
              <div class="text-gray-900 dark:text-white">{{ volumeInfo()?.num_devices }}</div>
            </div>
            <div>
              <div class="text-sm text-gray-500 dark:text-gray-400">Generation</div>
              <div class="text-gray-900 dark:text-white">{{ volumeInfo()?.generation }}</div>
            </div>
          </div>

          <!-- Usage bar -->
          <div class="mt-4">
            <div class="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div 
                class="h-full bg-blue-500 rounded-full transition-all"
                [style.width.%]="usagePercent()"
              ></div>
            </div>
            <div class="mt-1 text-sm text-gray-500 dark:text-gray-400 text-right">
              {{ usagePercent().toFixed(1) }}% used
            </div>
          </div>
        </div>
      }
    </div>
  `,
})
export class DeviceBrowserComponent implements OnInit {
  readonly tauri = inject(TauriService);

  devices = signal<DeviceInfo[]>([]);
  selectedDevice = signal<DeviceInfo | null>(null);
  volumeInfo = signal<VolumeInfo | null>(null);

  ngOnInit() {
    this.refreshDevices();
  }

  async refreshDevices() {
    const devices = await this.tauri.listDevices();
    
    // Check each device for BTRFS
    for (const device of devices) {
      device.is_btrfs = await this.tauri.detectBtrfs(device.path);
    }
    
    this.devices.set(devices);
  }

  async selectDevice(device: DeviceInfo) {
    this.selectedDevice.set(device);
    
    if (device.is_btrfs) {
      try {
        const info = await this.tauri.getVolumeInfo(device.path);
        this.volumeInfo.set(info);
      } catch (e) {
        console.error('Failed to get volume info:', e);
        this.volumeInfo.set(null);
      }
    } else {
      this.volumeInfo.set(null);
    }
  }

  usagePercent(): number {
    const info = this.volumeInfo();
    if (!info || info.total_bytes === 0) return 0;
    return (info.bytes_used / info.total_bytes) * 100;
  }
}
