import { Component, OnInit } from '@angular/core';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { OperationService } from '../operation.service';

@Component({
  selector: 'app-dashboard2',
  templateUrl: './dashboard2.component.html',
  styleUrls: ['./dashboard2.component.sass']
})
export class Dashboard2Component implements OnInit {
  version: string;
  status: string;
  playerCount: number;
  selectedSave: string;
  createSaveName: string;
  installVersionString: string;

  saves: string[];

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
    this.saves = [];
  }

  ngOnInit(): void {
    this.internalUpdate();
  }

  private internalUpdate(): void {
    this.apiClient.serverControlGet().subscribe(s => {
      this.status = s.game_status;
      this.playerCount = s.player_count;
    });
    this.apiClient.serverInstallGet().subscribe(s => {
      this.version = s.version;
    });
    this.apiClient.serverSavefileGet().subscribe(s => {
      this.saves = s.map(x => x.name);
    });
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
      savefile_id: this.createSaveName,
    };
    this.apiClient.serverSavefileSavefileIdPut$Response(payload).subscribe(resp => {
      const location = resp.headers.get('Location');
      if (location !== null) {
        this.operationService.subscribe(
          location,
          `Create save "${payload.savefile_id}"`,
          async () => {
            console.log('Create save success');
          },
          async err => {
            console.log(`Create save error: ${err}`);
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
            console.log('Install version success');
          },
          async err => {
            console.log(`Install version error: ${err}`);
          }
        );
      }
    });
  }

}
