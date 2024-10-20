import { ComponentFixture, TestBed } from '@angular/core/testing';

import { DlcListComponent } from './dlc-list.component';

describe('DlcListComponent', () => {
  let component: DlcListComponent;
  let fixture: ComponentFixture<DlcListComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [DlcListComponent]
    })
    .compileComponents();
    
    fixture = TestBed.createComponent(DlcListComponent);
    component = fixture.componentInstance;
    fixture.detectChanges();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
