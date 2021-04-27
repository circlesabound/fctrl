import { Component, OnInit } from '@angular/core';
import { faCogs, faUserCheck, faUserShield, faUserTimes } from '@fortawesome/free-solid-svg-icons';

@Component({
  selector: 'app-server-config',
  templateUrl: './server-config.component.html',
  styleUrls: ['./server-config.component.sass']
})
export class ServerConfigComponent implements OnInit {
  subnavServerSettingsIcon = faCogs;
  subnavAdminListIcon = faUserShield;
  subnavBanListIcon = faUserTimes;
  subnavWhiteListIcon = faUserCheck;

  constructor() { }

  ngOnInit(): void {
  }

}