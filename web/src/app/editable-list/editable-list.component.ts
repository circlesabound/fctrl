import { Component, EventEmitter, Input, Output } from '@angular/core';
import { faPlus, faTimes } from '@fortawesome/free-solid-svg-icons';

@Component({
  selector: 'app-editable-list',
  templateUrl: './editable-list.component.html',
  styleUrls: ['./editable-list.component.sass']
})
export class EditableListComponent {
  @Input() list: string[] = [];
  @Output() listChange = new EventEmitter<string[]>();

  addText = '';

  addIcon = faPlus;
  removeIcon = faTimes;

  constructor() { }

  addItem(): void {
    this.list.push(this.addText);
    this.addText = '';
    this.listChange.emit(this.list);
    console.log(`this.list = ${this.list}`);
  }

  remove(itemToRemove: string): void {
    this.list.forEach((item, index) => {
      if (item === itemToRemove) {
        this.list.splice(index, 1);
      }
    });
    this.listChange.emit(this.list);
  }

  inputKeyUp(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      this.addItem();
    }
  }

}
