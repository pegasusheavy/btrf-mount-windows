import { Routes } from '@angular/router';

export const routes: Routes = [
  {
    path: '',
    loadComponent: () =>
      import('./components/mount-manager/mount-manager.component').then(
        (m) => m.MountManagerComponent
      ),
  },
  {
    path: 'devices',
    loadComponent: () =>
      import('./components/device-browser/device-browser.component').then(
        (m) => m.DeviceBrowserComponent
      ),
  },
  {
    path: 'subvolumes',
    loadComponent: () =>
      import('./components/subvolume-tree/subvolume-tree.component').then(
        (m) => m.SubvolumeTreeComponent
      ),
  },
  {
    path: 'snapshots',
    loadComponent: () =>
      import('./components/snapshot-panel/snapshot-panel.component').then(
        (m) => m.SnapshotPanelComponent
      ),
  },
];
