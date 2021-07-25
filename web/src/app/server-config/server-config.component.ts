import { Component } from '@angular/core';
import { faCogs, faKey, faUserCheck, faUserShield, faUserTimes } from '@fortawesome/free-solid-svg-icons';

@Component({
  selector: 'app-server-config',
  templateUrl: './server-config.component.html',
  styleUrls: ['./server-config.component.sass']
})
export class ServerConfigComponent {
  subnavServerSettingsIcon = faCogs;
  subnavSecretsIcon = faKey;
  subnavAdminListIcon = faUserShield;
  subnavBanListIcon = faUserTimes;
  subnavWhiteListIcon = faUserCheck;

  constructor() { }

}
