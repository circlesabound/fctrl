import { Injectable } from '@angular/core';
import { ActivatedRouteSnapshot, CanActivate, Router, RouterStateSnapshot, UrlTree } from '@angular/router';
import { Observable } from 'rxjs';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { AuthDiscordService } from './auth-discord.service';
import { first, map } from 'rxjs/operators';
import { AuthInfoService } from './auth-info.service';

@Injectable({
  providedIn: 'root'
})
export class AuthGuard implements CanActivate {

  constructor(
    private apiClient: MgmtServerRestApiService,
    private authDiscordService: AuthDiscordService,
    private authInfoService: AuthInfoService,
    private router: Router) { }

  canActivate(
    route: ActivatedRouteSnapshot,
    state: RouterStateSnapshot): Observable<boolean | UrlTree> | Promise<boolean | UrlTree> | boolean | UrlTree {
    console.log('AuthGuard#canActivate called');
    return this.authInfoService.authRequirement.pipe(
      first(),
      map(req => {
        switch (req.kind) {
          case 'None':
            return true;
          case 'Discord':
            let accessTokenOpt = this.authDiscordService.tryGetAccessToken();
            if (accessTokenOpt.isSome()) {
              return true;
            } else {
              return this.router.parseUrl('/login');
            }
        }
      }),
    );
  }

}
