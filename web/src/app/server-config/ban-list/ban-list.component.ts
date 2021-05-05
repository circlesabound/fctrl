import { Component, OnInit } from '@angular/core';
import { faCheck, faSave } from '@fortawesome/free-solid-svg-icons';
import { delay, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';

@Component({
  selector: 'app-ban-list',
  templateUrl: './ban-list.component.html',
  styleUrls: ['./ban-list.component.sass']
})
export class BanListComponent implements OnInit {
  banList: string[] = [];

  saveButtonLoading = false;
  showTickIcon = false;
  saveIcon = faSave;
  tickIcon = faCheck;

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) { }

  ngOnInit(): void {
    this.fetchBanList();
  }

  fetchBanList(): void {
    this.apiClient.serverConfigBanlistGet().subscribe(bl => {
      this.banList = bl;
    });
  }

  pushBanList(): void {
    this.saveButtonLoading = true;
    this.apiClient.serverConfigBanlistPut({
      body: this.banList
    }).pipe(
      tap(() => {
        console.log('pushBanList returned');
        this.saveButtonLoading = false;
        this.showTickIcon = true;
      }),
      delay(3000),
    ).subscribe(() => {
      this.showTickIcon = false;
    });
  }

}
