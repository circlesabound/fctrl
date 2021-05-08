import { ComponentFixture, TestBed } from '@angular/core/testing';

import { ModObjectComponent } from './mod-object.component';

describe('ModObjectComponent', () => {
  let component: ModObjectComponent;
  let fixture: ComponentFixture<ModObjectComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      declarations: [ ModObjectComponent ]
    })
    .compileComponents();
  });

  beforeEach(() => {
    fixture = TestBed.createComponent(ModObjectComponent);
    component = fixture.componentInstance;
    fixture.detectChanges();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
