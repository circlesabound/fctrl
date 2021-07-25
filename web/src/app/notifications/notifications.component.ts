import { Component } from '@angular/core';
import { OperationService } from '../operation.service';

@Component({
  selector: 'app-notifications',
  templateUrl: './notifications.component.html',
  styleUrls: ['./notifications.component.sass']
})
export class NotificationsComponent {
  notificationsExpanded = false;
  operationService: OperationService;

  constructor(
    operationService: OperationService,
  ) {
    this.operationService = operationService;
  }

}
