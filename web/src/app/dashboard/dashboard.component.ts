import { Component, OnDestroy, OnInit } from '@angular/core';
import { Option } from 'prelude-ts';
import { SavefileObject, ServerControlStartPostRequest, ServerControlStatus } from '../mgmt-server-rest-api/models';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';
import { faAngleDown, faPlay, faPlus, faStop } from '@fortawesome/free-solid-svg-icons';
import { Observable, Subscription, timer } from 'rxjs';
import { switchMap, tap } from 'rxjs/operators';

@Component({
  selector: 'app-dashboard',
  templateUrl: './dashboard.component.html',
  styleUrls: ['./dashboard.component.sass']
})
export class DashboardComponent implements OnInit, OnDestroy {
  status: Option<ServerControlStatus>;

  savefiles: SavefileObject[];
  selectedSavefile: Option<SavefileObject>;
  savefileDropdownHidden: boolean;

  startServerButtonDisabled: boolean;
  stopServerButtonDisabled: boolean;

  dropdownArrowIcon = faAngleDown;
  startServerIcon = faPlay;
  stopServerIcon = faStop;
  createSaveIcon = faPlus;

  updateStatusSubscription: Option<Subscription>;

  createSaveName: string;
  upgradeVersionString: string;

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) {
    this.status = Option.none();
    this.savefiles = [];
    this.selectedSavefile = Option.none();
    this.savefileDropdownHidden = true;

    this.startServerButtonDisabled = false;
    this.stopServerButtonDisabled = false;

    this.updateStatusSubscription = Option.none();

    this.createSaveName = '';
    this.upgradeVersionString = '';
  }

  ngOnInit(): void {
    this.updateSavefiles();
    this.updateStatus();

    this.updateStatusSubscription = Option.some(timer(0, 5000).subscribe(() => {
      this.updateStatus();
    }));
  }

  ngOnDestroy(): void {
    this.updateStatusSubscription.map(s => s.unsubscribe());
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
    this.updateStartStopButtonState();
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
    });
  }

  createSave(): void {
    const params = {
      savefile_id: this.createSaveName,
    };
    this.apiClient.serverSavefileSavefileIdPut$Response(params).subscribe(response => {
      const location = response.headers.get('Location');
      console.log(`got location header value: ${location}`);
    });
  }

  private updateStatus(): void {
    console.log('updateStatus');
    this.apiClient.serverControlGet().subscribe(s => {
      this.status = Option.some(s);
      this.updateStartStopButtonState();
    });
  }

  private updateSavefiles(): void {
    this.apiClient.serverSavefileGet().subscribe(
      response => {
        this.savefiles = response;
      }
    );
  }

  private updateStartStopButtonState(): void {
    this.status.map(s => {
      if (s.game_status === 'NotRunning') {
        this.startServerButtonDisabled = this.selectedSavefile.isNone();
        this.stopServerButtonDisabled = true;
      } else {
        this.startServerButtonDisabled = true;
        this.stopServerButtonDisabled = false;
      }
    });
  }
}
