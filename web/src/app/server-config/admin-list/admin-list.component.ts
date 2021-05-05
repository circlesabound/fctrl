import { Component, OnInit } from '@angular/core';
import { faCheck, faSave } from '@fortawesome/free-solid-svg-icons';
import { delay, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';

@Component({
  selector: 'app-admin-list',
  templateUrl: './admin-list.component.html',
  styleUrls: ['./admin-list.component.sass']
})
export class AdminListComponent implements OnInit {
  adminList: string[] = [];

  saveButtonLoading = false;
  showTickIcon = false;
  saveIcon = faSave;
  tickIcon = faCheck;

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) { }

  ngOnInit(): void {
    this.fetchAdminList();
  }

  fetchAdminList(): void {
    this.apiClient.serverConfigAdminlistGet().subscribe(al => {
      this.adminList = al;
    });
  }

  pushAdminList(): void {
    this.saveButtonLoading = true;
    this.apiClient.serverConfigAdminlistPut({
      body: this.adminList
    }).pipe(
      tap(() => {
        console.log('pushAdminList returned');
        this.saveButtonLoading = false;
        this.showTickIcon = true;
      }),
      delay(3000),
    ).subscribe(() => {
      this.showTickIcon = false;
    });
  }

}
