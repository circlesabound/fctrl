import { Component, EventEmitter, Input, OnInit, Output } from '@angular/core';
import { faAngleDown, faTimes } from '@fortawesome/free-solid-svg-icons';
import { Option } from 'prelude-ts';
import { ModInfo } from '../mod-info';

@Component({
  // tslint:disable-next-line: component-selector
  selector: '[app-mod-object]',
  templateUrl: './mod-object.component.html',
  styleUrls: ['./mod-object.component.sass']
})
export class ModObjectComponent implements OnInit {
  @Input() modInfo!: ModInfo;
  @Output() modInfoChange = new EventEmitter<ModInfo>();
  @Output() delete = new EventEmitter<ModInfo>();

  dropdownArrowIcon = faAngleDown;
  removeIcon = faTimes;

  dropdownHidden = true;

  constructor() { }

  ngOnInit(): void {
    if (this.modInfo === null) {
      throw new Error('modInfo should not be null');
    }
  }

  removeSelf(): void {
    this.delete.emit(this.modInfo);
  }

  setSelectedVersion(version: string): void {
    this.modInfo.selectedVersion = version;
    this.modInfoChange.emit(this.modInfo);
  }

}
