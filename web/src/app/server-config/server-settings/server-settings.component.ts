import { Component, OnInit } from '@angular/core';
import { faCheck, faSave } from '@fortawesome/free-solid-svg-icons';
import { delay, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';

@Component({
  selector: 'app-server-settings',
  templateUrl: './server-settings.component.html',
  styleUrls: ['./server-settings.component.sass']
})
export class ServerSettingsComponent implements OnInit {
  text = '';

  saveButtonLoading = false;
  showTickIcon = false;
  saveIcon = faSave;
  tickIcon = faCheck;

  monacoOptions = {
    theme: 'vs-light',
    language: 'json',
  };

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) { }

  ngOnInit(): void {
    this.fetchServerSettings();
  }

  fetchServerSettings(): void {
    this.apiClient.serverConfigServerSettingsGet().subscribe(ss => {
      this.text = JSON.stringify(ss, null, 2);
    });
  }

  pushServerSettings(): void {
    this.saveButtonLoading = true;
    this.apiClient.serverConfigServerSettingsPut({
      body: JSON.parse(this.text),
    }).pipe(
      tap(() => {
        console.log('pushServerSettings returned');
        this.saveButtonLoading = false;
        this.showTickIcon = true;
      }),
      delay(3000),
    ).subscribe(() => {
      this.showTickIcon = false;
    });
  }

}
