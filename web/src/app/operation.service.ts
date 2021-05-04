import { Injectable } from '@angular/core';
import { webSocket } from 'rxjs/webSocket';
import { OperationStatus, ResponseWithId } from './schemas';

@Injectable({
  providedIn: 'root'
})
export class OperationService {
  notifications: string[] = [];

  constructor() { }

  subscribe(
    wsUrl: string,
    friendlyName: string,
    successCallback: () => void,
    errorCallback: (error: string) => void
  ): void {
    const ws = webSocket(wsUrl);
    ws.subscribe(
      msgUntyped => {
        const msg = msgUntyped as ResponseWithId;
        this.addNotification(`"${friendlyName}": ${JSON.stringify(msg.content)}`);
        if (msg.status === OperationStatus.Completed || msg.status === OperationStatus.Failed) {
          ws.complete();
        }
      },
      err => {
        this.addNotification(`"${friendlyName}" failed: WebSocket Error: ${err}`);
        console.error(`ws error: ${err}`);
        ws.complete();
      },
      () => {
        // closing
      },
    );
  }

  addNotification(message: string): void {
    this.notifications.push(message);
  }

  clearNotifications(): void {
    this.notifications = [];
  }
}
