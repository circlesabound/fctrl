import { Injectable } from '@angular/core';
import { Option } from 'prelude-ts';
import { ActivatedRouteSnapshot, CanActivate, Router, RouterStateSnapshot, UrlTree } from '@angular/router';
import { Observable } from 'rxjs';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { AuthDiscordService } from './auth-discord.service';

@Injectable({
  providedIn: 'root'
})
export class AuthGuard implements CanActivate {
  private skip = false;
  private discordOauthClientId: Option<string> = Option.none();

  constructor(
    private apiClient: MgmtServerRestApiService,
    private authDiscordService: AuthDiscordService,
    private router: Router) {
    // Get the type of auth required
    apiClient.authInfoGet().subscribe(ai => {
      if (ai.provider === 'none') {
        console.log('auth provider configured as none - all auth will be skipped');
        this.skip = true;
      } else if (ai.provider === 'discord' && ai.discord) {
        this.discordOauthClientId = Option.some(ai.discord.client_id);
      }
    });
  }

  canActivate(
    route: ActivatedRouteSnapshot,
    state: RouterStateSnapshot): Observable<boolean | UrlTree> | Promise<boolean | UrlTree> | boolean | UrlTree {
    console.log('AuthGuard#canActivate called');
    if (this.skip) {
      return true;
    }

    // TODO
    let accessTokenOpt = this.authDiscordService.tryGetAccessToken();
    if (accessTokenOpt.isNone()) {
      this.discordOauthClientId.ifSome(clientId => {
        let loginUrl = this.authDiscordService.getAuthorisationUrl(clientId);
        console.log(`login url is ${loginUrl}`);
      });
    }
    return true;
  }
}
