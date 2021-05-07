import { Component, OnInit } from '@angular/core';
import { faCheck, faSave } from '@fortawesome/free-solid-svg-icons';
import { delay, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';

@Component({
  selector: 'app-mod-settings',
  templateUrl: './mod-settings.component.html',
  styleUrls: ['./mod-settings.component.sass']
})
export class ModSettingsComponent implements OnInit {
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
    this.fetchModSettings();
  }

  fetchModSettings(): void {
    this.apiClient.serverModsSettingsGet().subscribe(ss => {
      this.text = JSON.stringify(ss, null, 2);
    });
  }

  pushModSettings(): void {
    this.saveButtonLoading = true;
    this.apiClient.serverModsSettingsPut({
      body: JSON.parse(this.text),
    }).pipe(
      tap(() => {
        console.log('pushModSettings returned');
        this.saveButtonLoading = false;
        this.showTickIcon = true;
      }),
      delay(3000),
    ).subscribe(() => {
      this.showTickIcon = false;
    });
  }

}
