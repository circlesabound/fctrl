import { Component, OnInit } from '@angular/core';
import { Router } from '@angular/router';
import { AuthDiscordService } from '../auth-discord.service';
import { AuthInfoService } from '../auth-info.service';

@Component({
  selector: 'app-login',
  templateUrl: './login.component.html',
  styleUrls: ['./login.component.sass']
})
export class LoginComponent implements OnInit {
  codeUrl = '#';

  constructor(
    private authDiscordService: AuthDiscordService,
    private authInfoService: AuthInfoService,
    private router: Router,
  ) { }

  ngOnInit(): void {
    this.authInfoService.authRequirement.subscribe(req => {
      switch (req.kind) {
        case 'None':
          // no need to log in, redirect
          this.redirectToRoot();
          break;
        case 'Discord':
          // check if logged in
          if (this.authDiscordService.tryGetAccessToken().isSome()) {
            this.redirectToRoot();
          } else {
            let clientId = req.clientId;
            this.codeUrl = this.authDiscordService.getAuthorisationUrl(clientId);
          }
      }
    });
  }

  private redirectToRoot(): void {
    this.router.navigate(['/']);
  }

}
