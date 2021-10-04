import { Component } from '@angular/core';
import { Router } from '@angular/router';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';
import { AuthDiscordService } from '../auth-discord.service';
import { AuthInfoService } from '../auth-info.service';

@Component({
  selector: 'app-logout',
  templateUrl: './logout.component.html',
  styleUrls: ['./logout.component.sass']
})
export class LogoutComponent {

  constructor(
    private authDiscordService: AuthDiscordService,
    private authInfoService: AuthInfoService,
    private router: Router,
  ) { }

  shouldDisplay(): Observable<boolean> {
    return this.authInfoService.authRequirement.pipe(
      map(req => {
        switch (req.kind) {
          case 'None':
            return false;
          case 'Discord':
            return this.authDiscordService.tryGetAccessToken().isSome();
        }
      })
    );
  }

  logout(): void {
    this.authDiscordService.logout();
    this.router.navigate(['/login']);
  }

}
