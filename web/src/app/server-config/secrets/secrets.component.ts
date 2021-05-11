import { Component, OnInit } from '@angular/core';
import { faSave, faCheck } from '@fortawesome/free-solid-svg-icons';
import { delay, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';

@Component({
  selector: 'app-secrets',
  templateUrl: './secrets.component.html',
  styleUrls: ['./secrets.component.sass']
})
export class SecretsComponent implements OnInit {
  username = '';
  token = '';

  saveButtonLoading = false;
  showTickIcon = false;
  saveIcon = faSave;
  tickIcon = faCheck;

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) { }

  ngOnInit(): void {
    this.fetchSecrets();
  }

  fetchSecrets(): void {
    this.apiClient.serverConfigSecretsGet().subscribe(s => {
      this.username = s.username;
    });
  }

  pushSecrets(): void {
    this.saveButtonLoading = true;
    this.apiClient.serverConfigSecretsPut({
      body: {
        username: this.username,
        token: this.token,
      }
    }).pipe(
      tap(() => {
        console.log('pushSecrets returned');
        this.saveButtonLoading = false;
        this.showTickIcon = true;
      }),
      delay(3000),
    ).subscribe(() => {
      this.showTickIcon = false;
    });
  }

}
