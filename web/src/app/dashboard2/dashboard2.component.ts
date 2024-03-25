import { Component, OnInit } from '@angular/core';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { OperationService } from '../operation.service';
import { MatSelectChange } from '@angular/material/select';

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
  saveDownloadHref: string;
  saveDownloadName: string;
  saveIsSelected: boolean;

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
    this.saveDownloadHref = '';
    this.saveDownloadName = '';
    this.saveIsSelected = false;
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

  saveSelectionChange(selectChangeEvent: MatSelectChange): void {
    // update download link
    this.saveIsSelected = true;
    this.saveDownloadName = selectChangeEvent.value + ".zip";
    this.saveDownloadHref = "/server/savefiles/" + this.saveDownloadName;
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
