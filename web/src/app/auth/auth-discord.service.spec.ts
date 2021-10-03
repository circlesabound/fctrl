import { TestBed } from '@angular/core/testing';

import { AuthDiscordService } from './auth-discord.service';

describe('AuthDiscordService', () => {
  let service: AuthDiscordService;

  beforeEach(() => {
    TestBed.configureTestingModule({});
    service = TestBed.inject(AuthDiscordService);
  });

  it('should be created', () => {
    expect(service).toBeTruthy();
  });
});
