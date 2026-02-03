import { Component, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { UpdaterService } from '../../services/updater.service';

@Component({
  selector: 'app-update-notification',
  standalone: true,
  imports: [CommonModule],
  template: `
    @if (updater.updateAvailable()) {
      <div class="update-notification">
        <div class="update-content">
          <div class="update-icon">
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
              <path stroke-linecap="round" stroke-linejoin="round" d="M9 8.25H7.5a2.25 2.25 0 0 0-2.25 2.25v9a2.25 2.25 0 0 0 2.25 2.25h9a2.25 2.25 0 0 0 2.25-2.25v-9a2.25 2.25 0 0 0-2.25-2.25H15M9 12l3 3m0 0 3-3m-3 3V2.25" />
            </svg>
          </div>
          <div class="update-text">
            <strong>Update Available</strong>
            @if (updater.updateInfo(); as info) {
              <span>Version {{ info.version }} is ready to install</span>
            }
          </div>
        </div>
        <div class="update-actions">
          @if (updater.installing()) {
            <span class="installing">Installing...</span>
          } @else {
            <button class="btn-install" (click)="install()">
              Install & Restart
            </button>
            <button class="btn-dismiss" (click)="dismiss()">
              Later
            </button>
          }
        </div>
      </div>
    }
  `,
  styles: [`
    .update-notification {
      position: fixed;
      bottom: 1rem;
      right: 1rem;
      background: linear-gradient(135deg, #1e40af 0%, #3b82f6 100%);
      color: white;
      padding: 1rem 1.25rem;
      border-radius: 0.75rem;
      box-shadow: 0 10px 25px rgba(0, 0, 0, 0.3);
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
      max-width: 320px;
      z-index: 1000;
      animation: slideIn 0.3s ease-out;
    }

    @keyframes slideIn {
      from {
        transform: translateY(100%);
        opacity: 0;
      }
      to {
        transform: translateY(0);
        opacity: 1;
      }
    }

    .update-content {
      display: flex;
      align-items: flex-start;
      gap: 0.75rem;
    }

    .update-icon {
      flex-shrink: 0;
      width: 2rem;
      height: 2rem;
      background: rgba(255, 255, 255, 0.2);
      border-radius: 0.5rem;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .update-icon svg {
      width: 1.25rem;
      height: 1.25rem;
    }

    .update-text {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
    }

    .update-text strong {
      font-size: 0.95rem;
    }

    .update-text span {
      font-size: 0.8rem;
      opacity: 0.9;
    }

    .update-actions {
      display: flex;
      gap: 0.5rem;
      justify-content: flex-end;
    }

    .btn-install {
      background: white;
      color: #1e40af;
      border: none;
      padding: 0.5rem 1rem;
      border-radius: 0.5rem;
      font-weight: 600;
      font-size: 0.85rem;
      cursor: pointer;
      transition: transform 0.15s, box-shadow 0.15s;
    }

    .btn-install:hover {
      transform: translateY(-1px);
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
    }

    .btn-dismiss {
      background: transparent;
      color: white;
      border: 1px solid rgba(255, 255, 255, 0.4);
      padding: 0.5rem 1rem;
      border-radius: 0.5rem;
      font-size: 0.85rem;
      cursor: pointer;
      transition: background 0.15s;
    }

    .btn-dismiss:hover {
      background: rgba(255, 255, 255, 0.1);
    }

    .installing {
      font-size: 0.85rem;
      opacity: 0.9;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }

    .installing::before {
      content: '';
      width: 1rem;
      height: 1rem;
      border: 2px solid rgba(255, 255, 255, 0.3);
      border-top-color: white;
      border-radius: 50%;
      animation: spin 0.8s linear infinite;
    }

    @keyframes spin {
      to {
        transform: rotate(360deg);
      }
    }
  `]
})
export class UpdateNotificationComponent {
  readonly updater = inject(UpdaterService);

  install(): void {
    this.updater.installUpdate();
  }

  dismiss(): void {
    this.updater.dismissUpdate();
  }
}
