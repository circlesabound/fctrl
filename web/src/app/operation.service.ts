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
        this.add_notification(`"${friendlyName}": ${JSON.stringify(msg.content)}`);
        if (msg.status === OperationStatus.Completed || msg.status === OperationStatus.Failed) {
          ws.complete();
        }
      },
      err => {
        this.add_notification(`"${friendlyName}" failed: WebSocket Error: ${err}`);
        console.error(`ws error: ${err}`);
        ws.complete();
      },
      () => {
        // closing
      },
    );
  }

  add_notification(message: string): void {
    this.notifications.push(message);
  }

  clear_notifications(): void {
    this.notifications = [];
  }
}
