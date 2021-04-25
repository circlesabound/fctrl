import { Component, OnInit } from '@angular/core';
import { faList, faTools } from '@fortawesome/free-solid-svg-icons';

@Component({
  selector: 'app-mods',
  templateUrl: './mods.component.html',
  styleUrls: ['./mods.component.sass']
})
export class ModsComponent implements OnInit {
  subnavModListIcon = faList;
  subnavModSettingsIcon = faTools;

  constructor() { }

  ngOnInit(): void {
  }

}
