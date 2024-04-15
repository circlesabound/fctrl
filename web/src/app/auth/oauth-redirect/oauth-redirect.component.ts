import { Component, OnInit } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { AuthDiscordService } from '../auth-discord.service';

@Component({
  selector: 'app-oauth-redirect',
  templateUrl: './oauth-redirect.component.html',
  styleUrls: ['./oauth-redirect.component.sass']
})
export class OauthRedirectComponent implements OnInit {

  constructor(
    private router: Router,
    private route: ActivatedRoute,
    private authDiscordService: AuthDiscordService,
  ) { }

  ngOnInit(): void {
    let ss = this.route.snapshot;
    if (ss.queryParamMap.has('code')) {
      let code = ss.queryParamMap.get('code')!;
      this.authDiscordService.codeToToken(code).subscribe(s => {
        this.router.navigate(['dashboard']);
      });
    }
  }

}
