import { Component, OnInit } from '@angular/core';
import { faCheck, faSave, faExternalLink, faRefresh } from '@fortawesome/free-solid-svg-icons';
import { delay, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';
import { DlcName } from 'src/app/mgmt-server-rest-api/models';
import { DlcInfo } from './dlc-info';

@Component({
  selector: 'app-dlc-list',
  templateUrl: './dlc-list.component.html',
  styleUrls: ['./dlc-list.component.sass']
})
export class DlcListComponent implements OnInit {
  dlcInfoList: DlcInfo[] = [
    {
      name: DlcName.Base,
      title: 'Factorio Base Game',
      enabled: false,
    },
    {
      name: DlcName.ElevatedRails,
      title: 'Elevated Rails',
      enabled: false,
    },
    {
      name: DlcName.Quality,
      title: 'Quality',
      enabled: false,
    },
    {
      name: DlcName.SpaceAge,
      title: 'Space Age',
      enabled: false,
    },
  ];

  saveButtonLoading = false;
  saveShowTickIcon = false;
  syncButtonLoading = false;
  syncShowTickIcon = false;
  saveIcon = faSave;
  tickIcon = faCheck;
  linkIcon = faExternalLink;
  syncIcon = faRefresh;

  syncModalActive = false;
  syncSelectedSavename: string | null;
  savenames: string[];

  ready = false;

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) {
    this.syncSelectedSavename = null;
    this.savenames = [];
  }

  ngOnInit(): void {
    this.fetchDlcList();
  }

  fetchDlcList(): void {
    this.apiClient.serverModsDlcGet().subscribe(dlcList => {
      this.dlcInfoList.forEach(info => {
        if (dlcList.indexOf(info.name) != -1) {
          info.enabled = true;
        } else {
          info.enabled = false;
        }
      })
      this.ready = true;
    });
  }

  // TODO handle error
  pushDlcList(): void {
    this.saveButtonLoading = true;
    this.apiClient.serverModsDlcPut({
      body: this.dlcInfoList.filter(dlcInfo => dlcInfo.enabled).map(dlcInfo => dlcInfo.name).map(name => name as DlcName),
    }).pipe(
      tap(() => {
        console.log('pushDlcList returned');
        this.saveButtonLoading = false;
        this.saveShowTickIcon = true;
      }),
      delay(3000),
    ).subscribe(() => {
      this.saveShowTickIcon = false;
    });
  }
}
