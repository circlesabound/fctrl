import { Component, OnInit } from '@angular/core';
import { faCheck, faDownload, faSave, faUpload } from '@fortawesome/free-solid-svg-icons';
import { delay, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';
import { environment } from 'src/environments/environment';

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
  downloadIcon = faDownload;
  uploadIcon = faUpload;

  useEditor = false;

  monacoOptions = {
    theme: 'vs-light',
    language: 'json',
  };

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) { }

  ngOnInit(): void {
    if (this.useEditor) {
      this.fetchModSettings();
    }
  }

  downloadModSettingsDat(): void {
    this.apiClient.serverModsSettingsDatGet$Response().subscribe(resp => {
      const location = resp.headers.get('Location');
      if (location !== null) {
        const a = document.createElement('a');
        a.href = window.location.protocol + '//' + environment.backendHost + location;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
      }
    })
  }

  uploadModSettingsDat(): void {
    // TODO
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
