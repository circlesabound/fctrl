import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { Injectable } from '@angular/core';
import { ReplaySubject } from 'rxjs';

@Injectable({
  providedIn: 'root'
})
export class AuthInfoService {
  private authReqSource = new ReplaySubject<AuthRequirement>();
  authRequirement = this.authReqSource.asObservable();

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) {
    // Get the type of auth required
    apiClient.authInfoGet().subscribe(ai => {
      let val: AuthRequirement;
      if (ai.provider === 'none') {
        console.log('auth provider configured as none - all auth will be skipped');
        val = {
          kind: 'None',
        };
      } else if (ai.provider === 'discord' && ai.discord) {
        val = {
          kind: 'Discord',
          clientId: ai.discord.client_id,
        };
      } else {
        val = {
          kind: 'None',
        };
      }

      this.authReqSource.next(val);
    });
  }
}

export interface AuthTypeNone {
  kind: 'None';
}

export interface AuthTypeDiscord {
  kind: 'Discord';
  clientId: string;
}

export type AuthRequirement = AuthTypeNone | AuthTypeDiscord;
