import { ComponentFixture, TestBed } from '@angular/core/testing';

import { WhiteListComponent } from './white-list.component';

describe('WhiteListComponent', () => {
  let component: WhiteListComponent;
  let fixture: ComponentFixture<WhiteListComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      declarations: [ WhiteListComponent ]
    })
    .compileComponents();
  });

  beforeEach(() => {
    fixture = TestBed.createComponent(WhiteListComponent);
    component = fixture.componentInstance;
    fixture.detectChanges();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
