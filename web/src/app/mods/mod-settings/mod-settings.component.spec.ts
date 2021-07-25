import { ComponentFixture, TestBed } from '@angular/core/testing';

import { ModSettingsComponent } from './mod-settings.component';

describe('ModSettingsComponent', () => {
  let component: ModSettingsComponent;
  let fixture: ComponentFixture<ModSettingsComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      declarations: [ ModSettingsComponent ]
    })
      .compileComponents();
  });

  beforeEach(() => {
    fixture = TestBed.createComponent(ModSettingsComponent);
    component = fixture.componentInstance;
    fixture.detectChanges();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
