import { Component } from '@angular/core';
import { faComments, faStream } from '@fortawesome/free-solid-svg-icons';

@Component({
  selector: 'app-logs',
  templateUrl: './logs.component.html',
  styleUrls: ['./logs.component.sass']
})
export class LogsComponent {
  subnavSystemIcon = faStream;
  subnavChatIcon = faComments;

  constructor() { }

}
