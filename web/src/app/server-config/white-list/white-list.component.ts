import { Component, OnInit } from '@angular/core';
import { faCheck, faSave } from '@fortawesome/free-solid-svg-icons';
import { delay, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';

@Component({
  selector: 'app-white-list',
  templateUrl: './white-list.component.html',
  styleUrls: ['./white-list.component.sass']
})
export class WhiteListComponent implements OnInit {
  whiteList: string[] = [];
  useWhiteList = false;

  saveButtonLoading = false;
  showTickIcon = false;
  saveIcon = faSave;
  tickIcon = faCheck;

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) { }

  ngOnInit(): void {
    this.fetchWhiteList();
  }

  fetchWhiteList(): void {
    this.apiClient.serverConfigWhitelistGet().subscribe(wl => {
      this.useWhiteList = wl.enabled;
      this.whiteList = wl.users;
    });
  }

  pushWhiteList(): void {
    this.saveButtonLoading = true;
    this.apiClient.serverConfigWhitelistPut({
      body: {
        enabled: this.useWhiteList,
        users: this.whiteList,
      }
    }).pipe(
      tap(() => {
        console.log('pushWhiteList returned');
        this.saveButtonLoading = false;
        this.showTickIcon = true;
      }),
      delay(3000),
    ).subscribe(() => {
      this.showTickIcon = false;
    });
  }

}
