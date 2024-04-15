import { Component, OnInit } from '@angular/core';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { OperationService } from '../operation.service';
import { MatSelectChange } from '@angular/material/select';
import { environment } from 'src/environments/environment';

@Component({
  selector: 'app-dashboard2',
  templateUrl: './dashboard2.component.html',
  styleUrls: ['./dashboard2.component.sass'],
})
export class Dashboard2Component implements OnInit {
  version: string;
  status: string;
  playerCount: number;
  selectedSave: string;
  createSaveName: string;
  installVersionString: string;

  downloadAvailableVersions: string[] = [];
  saves: string[] = [];

  constructor(
    private apiClient: MgmtServerRestApiService,
    private operationService: OperationService,
  ) {
    this.version = 'not installed';
    this.status = '';
    this.playerCount = 0;
    this.selectedSave = '';
    this.createSaveName = '';
    this.installVersionString = '';
  }

  ngOnInit(): void {
    this.internalUpdateAll();
  }

  private internalUpdateAll(): void {
    this.apiClient.serverControlGet().subscribe(s => {
      this.status = s.game_status;
      this.playerCount = s.player_count;
    });
    this.internalUpdateVersion();
    this.apiClient.serverSavefilesGet().subscribe(s => {
      this.saves = s.map(x => x.name);
    });
  }

  private internalUpdateVersion(): void {
    this.apiClient.serverInstallGet().subscribe(s => {
      this.version = s.version;
    });
  }

  private internalUpdateAvailableVersions(): void {
    // TODO
  }

  startServer(): void {
    if (this.selectedSave) {
      const payload = {
        body: {
          savefile: this.selectedSave,
        }
      };
      this.apiClient.serverControlStartPost(payload).subscribe(_ => {
        console.log('startServer returned');
      });
    }
  }

  stopServer(): void {
    this.apiClient.serverControlStopPost().subscribe(_ => {
      console.log('stopServer returned');
    });
  }

  createSave(): void {
    const payload = {
      body: {
        savefile: this.createSaveName,
      },
    };
    this.apiClient.serverControlCreatePost$Response(payload).subscribe(resp => {
      const location = resp.headers.get('Location');
      if (location !== null) {
        this.operationService.subscribe(
          location,
          `Create save "${payload.body.savefile}"`,
          async () => {
            console.debug('Create save success');
          },
          async err => {
            console.warn(`Create save error: ${err}`);
          }
        );
      }
    });
  }

  downloadSave(): void {
    // 2-part download process to allow us to use native browser download experience
    // first, we generate a temporary download link that does not require authentication token
    this.apiClient.serverSavefilesSavefileIdGet$Response({ savefile_id: this.selectedSave }).subscribe(resp => {
      const location = resp.headers.get('Location');
      if (location !== null) {
        // then we create an invisible element with that link destination and trigger a click on it
        const a = document.createElement('a');
        a.href = window.location.protocol + '//' + environment.backendHost + location;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
      }
    })
  }

  deleteSave(): void {
    // TODO no endpoint for this
  }

  installVersion(): void {
    const payload = {
      body: {
        version: this.installVersionString,
        force_install: false,
      }
    };
    this.apiClient.serverInstallPost$Response(payload).subscribe(resp => {
      const location = resp.headers.get('Location');
      if (location !== null) {
        this.operationService.subscribe(
          location,
          `Install version ${payload.body.version}`,
          async () => {
            console.debug('Install version success');
            this.internalUpdateVersion();
          },
          async err => {
            console.warn(`Install version error: ${err}`);
            this.internalUpdateVersion();
          }
        );
      }
    });
  }

}
