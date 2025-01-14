import { Component, OnInit } from '@angular/core';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { OperationService } from '../operation.service';
import { environment } from 'src/environments/environment';
import { faCheck, faFileCirclePlus, faUpload } from '@fortawesome/free-solid-svg-icons';
import { delay, switchMap, tap } from 'rxjs/operators';
import { concat, interval, of } from 'rxjs';

@Component({
  selector: 'app-dashboard2',
  templateUrl: './dashboard2.component.html',
  styleUrls: ['./dashboard2.component.sass'],
})
export class Dashboard2Component implements OnInit {
  installedVersion: string | null;
  status: string;
  playerCount: number;
  selectedSave: string;
  installVersionString: string;

  uploadSavefileButtonLoading = false;
  uploadSavefileButtonShowTickIcon = false;
  uploadIcon = faUpload;
  tickIcon = faCheck;

  downloadAvailableVersions: string[] = [];
  saves: string[] = [];

  savefileToUpload: File | null;

  createSaveIcon = faFileCirclePlus;
  createSaveModalActive = false;
  createSaveName: string | null;

  cpuTotal: number | null = null;
  cpus: number[] = [];
  mem_used_prct: number | null = null;

  constructor(
    private apiClient: MgmtServerRestApiService,
    private operationService: OperationService,
  ) {
    this.installedVersion = null;
    this.status = '';
    this.playerCount = 0;
    this.selectedSave = '';
    this.createSaveName = '';
    this.installVersionString = '';
    this.savefileToUpload = null;
  }

  ngOnInit(): void {
    this.internalUpdateAll();
    interval(15000).subscribe(_ => this.internalUpdateSystemResources());
  }

  private internalUpdateAll(): void {
    this.internalUpdateVersion();
    this.internalUpdateSaves();
    this.internalUpdateGameStatus();
    this.internalUpdateSystemResources();
  }

  private internalUpdateGameStatus(): void {
    this.apiClient.serverControlGet().subscribe(s => {
      this.status = s.game_status;
      this.playerCount = s.player_count;
    });
  }

  private internalUpdateSaves(): void {
    this.apiClient.serverSavefilesGet().subscribe(s => {
      this.saves = s.map(x => x.name);
    });
  }

  private internalUpdateVersion(): void {
    this.apiClient.serverInstallGet().subscribe(s => {
      this.installedVersion = s.version;
    });
  }

  private internalUpdateAvailableVersions(): void {
    // TODO implement for install version dropdown
  }

  private internalUpdateSystemResources(): void {
    this.apiClient.systemMonitorGet().subscribe(s => {
      this.cpuTotal = s.cpu_total;
      this.cpus = s.cpus;
      this.mem_used_prct = s.mem_used_bytes / s.mem_total_bytes * 100;
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

  createSave(savename: string): void {
    const payload = {
      body: {
        savefile: savename,
        map_gen_settings: undefined, // TODO
        map_settings: undefined, // TODO
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
            this.internalUpdateSaves();
          },
          async err => {
            console.warn(`Create save error: ${err}`);
          }
        );
      }
      this.createSaveModalActive = false;
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
    this.apiClient.serverSavefilesSavefileIdDelete({
      savefile_id: this.selectedSave
    }).subscribe({
      next: (_) => {
        this.internalUpdateSaves();
      },
      error: (e) => {
        alert('Error deleting save: ' + e);
      }
    })
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

  uploadSavefile(): void {
    if (this.savefileToUpload === null) {
      return;
    }

    this.uploadSavefileButtonLoading = true;

    // trim ".zip" from end of filename
    let savefile_id = this.savefileToUpload.name.split('.').slice(0, -1).join('.')

    let totalSize = this.savefileToUpload.size;
    let chunkSizeBytes = 2 * 1000 * 1000; // 2 MB
    let offset = 0;
    let offsetsObservableArray = [];

    while (offset <= totalSize) {
      console.debug("preparing observable for chunk from " + offset);
      let currentChunkSize = Math.min(chunkSizeBytes, totalSize - offset);

      if (currentChunkSize === 0) {
        // finalise with sentinel
        offsetsObservableArray.push(this.apiClient.serverSavefilesSavefileIdPut({
          body: new Blob(),
          savefile_id,
          "Content-Range": `bytes ${offset}-${offset}/${totalSize}`,
        }).pipe(
          tap({
            complete: () => this.uploadSavefileButtonShowTickIcon = true,
            error: e => alert('Error uploading save: ' + JSON.stringify(e)),
            finalize: () => this.uploadSavefileButtonLoading = false,
          }),
        ));
        console.debug("prepared sentinel");
        break;
      }

      let observable = of([offset, currentChunkSize]).pipe(
        switchMap(([offset, chunkSize]) => {
          let chunk = this.savefileToUpload!.slice(offset, offset + chunkSize);
          return this.apiClient.serverSavefilesSavefileIdPut({
            body: chunk,
            savefile_id,
            "Content-Range": `bytes ${offset}-${offset + chunkSize}/${totalSize}`,
          });
        }),
        tap({
          complete: () => this.uploadSavefileButtonShowTickIcon = true,
          error: e => alert('Error uploading save: ' + JSON.stringify(e)),
          finalize: () => this.uploadSavefileButtonLoading = false,
        }),
      );
      offsetsObservableArray.push(observable);
      offset += currentChunkSize;
    }

    // do them in order
    concat(...offsetsObservableArray).pipe(
      delay(3000),
    ).subscribe().add(() => {
      this.uploadSavefileButtonShowTickIcon = false
      this.internalUpdateSaves();
    });

  }
}
