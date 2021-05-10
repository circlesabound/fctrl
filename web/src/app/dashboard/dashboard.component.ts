import { Component, OnDestroy, OnInit } from '@angular/core';
import { Option } from 'prelude-ts';
import { SavefileObject, ServerControlStartPostRequest, ServerControlStatus } from '../mgmt-server-rest-api/models';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { faAngleDown, faPlay, faPlus, faStop } from '@fortawesome/free-solid-svg-icons';
import { Subject, Subscription, timer } from 'rxjs';
import { OperationService } from '../operation.service';
import { StatusControl } from './status-control';

@Component({
  selector: 'app-dashboard',
  templateUrl: './dashboard.component.html',
  styleUrls: ['./dashboard.component.sass']
})
export class DashboardComponent implements OnInit, OnDestroy {
  savefiles: SavefileObject[];
  selectedSavefile: Option<SavefileObject>;
  savefileDropdownHidden: boolean;

  startServerButtonDisabled: boolean;
  stopServerButtonDisabled: boolean;

  dropdownArrowIcon = faAngleDown;
  startServerIcon = faPlay;
  stopServerIcon = faStop;
  createSaveIcon = faPlus;

  createSaveName: string;
  upgradeVersionString: string;

  manualUpdateStatusSubject = new Subject<void>();
  statusControl = StatusControl.Invalid;

  constructor(
    private apiClient: MgmtServerRestApiService,
    private operationService: OperationService,
  ) {
    this.savefiles = [];
    this.selectedSavefile = Option.none();
    this.savefileDropdownHidden = true;

    this.startServerButtonDisabled = false;
    this.stopServerButtonDisabled = false;

    this.createSaveName = '';
    this.upgradeVersionString = '';
  }

  ngOnInit(): void {
    this.updateSavefiles();
  }

  ngOnDestroy(): void {
    //
  }

  displaySavefileOpt(savefile: Option<SavefileObject>): string {
    return savefile.map(this.displaySavefile).getOrElse('Select savefile');
  }

  displaySavefile(savefile: SavefileObject): string {
    // return `${savefile.name} (${savefile.last_modified})`;
    return savefile.name;
  }

  toggleSavefileDropdown(): void {
    this.savefileDropdownHidden = !this.savefileDropdownHidden;
  }

  setSelectedSavefile(savefile: SavefileObject): void {
    this.selectedSavefile = Option.some(savefile);
    this.handleStatusControlEvent(this.statusControl);
  }

  startServer(): void {
    this.selectedSavefile.map(s => {
      const body: ServerControlStartPostRequest = {
        savefile: s.name,
      };
      this.apiClient.serverControlStartPost({
        body,
      }).subscribe(() => {
        console.log('startServer returned');
      });
    });
  }

  stopServer(): void {
    this.apiClient.serverControlStopPost().subscribe(() => {
      console.log('stopServer returned');
    });
  }

  upgradeInstall(): void {
    const params = {
      body: {
        version: this.upgradeVersionString,
        force_install: false,
      }
    };
    this.apiClient.serverInstallPost$Response(params).subscribe(response => {
      const location = response.headers.get('Location');
      console.log(`got location header value: ${location}`);

      if (location !== null) {
        this.operationService.subscribe(
          location,
          `Install Factorio ${params.body.version}`,
          async () => {
            // tba
          },
          async err => {
            // tba
          },
        );
      }
    });
  }

  createSave(): void {
    const params = {
      savefile_id: this.createSaveName,
    };
    this.apiClient.serverSavefileSavefileIdPut$Response(params).subscribe(response => {
      const location = response.headers.get('Location');
      console.log(`got location header value: ${location}`);

      if (location !== null) {
        this.operationService.subscribe(
          location,
          `Create save "${params.savefile_id}"`,
          async () => {
            // tba
          },
          async err => {
            // tba
          },
        );
      }
    });
  }

  handleStatusControlEvent(s: StatusControl): void {
    if (s === StatusControl.CanStart) {
      this.startServerButtonDisabled = this.selectedSavefile.isNone();
      this.stopServerButtonDisabled = true;
    } else if (s === StatusControl.CanStop) {
      this.startServerButtonDisabled = true;
      this.stopServerButtonDisabled = false;
    } else {
      this.startServerButtonDisabled = true;
      this.stopServerButtonDisabled = true;
    }

    this.statusControl = s;
  }

  private updateSavefiles(): void {
    this.apiClient.serverSavefileGet().subscribe(
      response => {
        this.savefiles = response;
      }
    );
  }
}
