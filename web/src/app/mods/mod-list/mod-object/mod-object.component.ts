import { Component, EventEmitter, Input, OnInit, Output } from '@angular/core';
import { faAngleDown, faTimes } from '@fortawesome/free-solid-svg-icons';
import { Option } from 'prelude-ts';
import { ModInfo } from '../mod-info';

@Component({
  // eslint-disable-next-line @angular-eslint/component-selector
  selector: '[app-mod-object]',
  templateUrl: './mod-object.component.html',
  styleUrls: ['./mod-object.component.sass']
})
export class ModObjectComponent implements OnInit {
  @Input() modInfo!: ModInfo;
  @Output() modInfoChange = new EventEmitter<ModInfo>();
  @Output() removeModInfo = new EventEmitter<ModInfo>();

  removeIcon = faTimes;

  constructor() { }

  ngOnInit(): void {
    if (this.modInfo === null) {
      throw new Error('modInfo should not be null');
    }
  }

  removeSelf(): void {
    this.removeModInfo.emit(this.modInfo);
  }

  // TODO possibly not needed anymore with ngModel bind, need to test
  setSelectedVersion(version: string): void {
    this.modInfo.selectedVersion = version;
    this.modInfoChange.emit(this.modInfo);
  }

}
