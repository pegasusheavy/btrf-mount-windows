import { Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterOutlet, RouterLink, RouterLinkActive } from '@angular/router';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [CommonModule, RouterOutlet, RouterLink, RouterLinkActive],
  template: `
    <div class="min-h-screen flex flex-col">
      <!-- Header -->
      <header class="bg-white dark:bg-gray-800 shadow-sm border-b border-gray-200 dark:border-gray-700">
        <div class="px-4 py-3 flex items-center justify-between">
          <div class="flex items-center gap-3">
            <svg class="w-8 h-8 text-blue-500" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"/>
            </svg>
            <h1 class="text-xl font-semibold text-gray-900 dark:text-white">
              BTRFS Mount Windows
            </h1>
          </div>
          
          <nav class="flex gap-1">
            <a routerLink="/" routerLinkActive="bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300"
               [routerLinkActiveOptions]="{exact: true}"
               class="px-4 py-2 rounded-lg text-sm font-medium text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors">
              Mount
            </a>
            <a routerLink="/devices" routerLinkActive="bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300"
               class="px-4 py-2 rounded-lg text-sm font-medium text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors">
              Devices
            </a>
            <a routerLink="/subvolumes" routerLinkActive="bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300"
               class="px-4 py-2 rounded-lg text-sm font-medium text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors">
              Subvolumes
            </a>
            <a routerLink="/snapshots" routerLinkActive="bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300"
               class="px-4 py-2 rounded-lg text-sm font-medium text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors">
              Snapshots
            </a>
          </nav>
        </div>
      </header>

      <!-- Main content -->
      <main class="flex-1 p-6">
        <router-outlet />
      </main>

      <!-- Footer -->
      <footer class="bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700 px-4 py-3">
        <div class="flex items-center justify-between text-sm text-gray-500 dark:text-gray-400">
          <span>BTRFS Mount Windows v0.1.0</span>
          <span>Pegasus Heavy Industries LLC</span>
        </div>
      </footer>
    </div>
  `,
})
export class AppComponent {
  title = signal('BTRFS Mount Windows');
}
