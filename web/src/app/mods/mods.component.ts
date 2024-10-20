import { Component } from '@angular/core';
import { faDownload, faList, faTools } from '@fortawesome/free-solid-svg-icons';

@Component({
  selector: 'app-mods',
  templateUrl: './mods.component.html',
  styleUrls: ['./mods.component.sass']
})
export class ModsComponent {
  subnavDlcIcon = faDownload;
  subnavModListIcon = faList;
  subnavModSettingsIcon = faTools;

  constructor() { }

}
