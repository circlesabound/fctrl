import { TestBed } from '@angular/core/testing';

import { AuthInfoService } from './auth-info.service';

describe('AuthInfoService', () => {
  let service: AuthInfoService;

  beforeEach(() => {
    TestBed.configureTestingModule({});
    service = TestBed.inject(AuthInfoService);
  });

  it('should be created', () => {
    expect(service).toBeTruthy();
  });
});
