import { Component, OnInit } from '@angular/core';
import { faCheck, faPlus, faSave } from '@fortawesome/free-solid-svg-icons';
import { Option } from 'prelude-ts';
import { Observable, of, Subject, timer } from 'rxjs';
import { catchError, debounceTime, distinctUntilChanged, map, switchMap, tap } from 'rxjs/operators';
import { ModInfoShort } from 'src/app/factorio-mod-portal-api/models';
import { FactorioModPortalApiService } from 'src/app/factorio-mod-portal-api/services';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';
import { OperationService } from 'src/app/operation.service';
import { ModInfo } from './mod-info';

@Component({
  selector: 'app-mod-list',
  templateUrl: './mod-list.component.html',
  styleUrls: ['./mod-list.component.sass']
})
export class ModListComponent implements OnInit {
  modInfoList: ModInfo[] = [];

  addModName = '';
  private addModNameSubject = new Subject<string>();
  addModPrefetch: Option<ModInfo> = Option.none();

  saveButtonLoading = false;
  showTickIcon = false;
  addIcon = faPlus;
  saveIcon = faSave;
  tickIcon = faCheck;

  ready = false;

  constructor(
    private apiClient: MgmtServerRestApiService,
    private modPortalClient: FactorioModPortalApiService,
    private operationService: OperationService,
  ) {}

  ngOnInit(): void {
    this.fetchModList();
    this.addModNameSubject.pipe(
      debounceTime(300),
      distinctUntilChanged(),
      switchMap((name) => this.prefetchModToAdd(name)),
    ).subscribe(mi => {
      this.addModPrefetch = Option.some(mi);
    });
  }

  fetchModList(): void {
    this.apiClient.serverModsListGet().subscribe(modList => {
      this.modPortalClient.apiModsGet({
        namelist: modList.map(mo => mo.name),
        page_size: 'max',
      }).subscribe(listResponse => {
        const infoList: ModInfo[] = [];
        for (const remoteInfo of listResponse.results) {
          infoList.push({
            name: remoteInfo.name ?? '<undefined>',
            title: remoteInfo.title ?? '<undefined>',
            summary: remoteInfo.summary ?? '<undefined>',
            selectedVersion: modList.find(mo => mo.name === remoteInfo.name)?.version ?? '',
            versions: remoteInfo.releases?.map(r => r.version).sort().reverse() ?? [],
          });
        }
        this.modInfoList = infoList.sort((lhs, rhs) => lhs.name.localeCompare(rhs.name));

        this.ready = true;
      });
    });
  }

  pushModList(): void {
    this.saveButtonLoading = true;

    this.apiClient.serverModsListPost$Response({
      body: this.modInfoList.map(info => {
        return {
          name: info.name,
          version: info.selectedVersion,
        };
      }),
    }).subscribe(response => {
      const location = response.headers.get('Location');

      if (location !== null) {
        this.operationService.subscribe(
          location,
          'Push mod list',
          async () => {
            console.log('push mod list succeeded');
            this.saveButtonLoading = false;
            this.showTickIcon = true;
            timer(3000).subscribe(_ => {
              this.showTickIcon = false;
            });
          },
          async err => {
            console.log(`push mod list failed: ${err}`);
            this.saveButtonLoading = false;
          }
        );
      } else {
        console.error('Location header was empty');
      }
    });
  }

  addMod(): void {
    this.addModPrefetch.ifSome(info => {
      this.modInfoList.push(info);
      this.modInfoList.sort((lhs, rhs) => lhs.name.localeCompare(rhs.name));
    });
    this.addModPrefetch = Option.none();
    this.addModName = '';
  }

  removeMod(modToRemove: ModInfo): void {
    const index = this.modInfoList.indexOf(modToRemove);
    if (index !== -1) {
      this.modInfoList.splice(index, 1);
    }
  }

  bufferedPrefetchModToAdd(name: string): void {
    if (name.trim() !== '') {
      this.addModNameSubject.next(name);
    }
  }

  prefetchModToAdd(name: string): Observable<ModInfo> {
    return this.modPortalClient.apiModsModNameGet({
      mod_name: name,
    }).pipe(
      catchError(err => {
        console.error(`error with prefetch: ${JSON.stringify(err, null, 2)}`);
        const ret: ModInfoShort = {
            name: '',
            title: '',
            downloads_count: 0,
            owner: '',
            summary: '',
            releases: [],
        };
        return of(ret);
      }),
      map(infoShort => {
        const versions = infoShort.releases.map(r => r.version).sort().reverse();
        const selectedVersion = versions.length === 0 ? '' : versions[0];
        const ret: ModInfo = {
          name: infoShort.name,
          title: infoShort.title,
          summary: infoShort.summary,
          versions,
          selectedVersion,
        };
        return ret;
      }
    ));
  }

  getVersionsToAdd(): string[] {
    return this.addModPrefetch.map(mi => mi.versions).getOrElse([]);
  }

  getSelectedVersionToAdd(): string {
    return this.addModPrefetch.map(mi => mi.selectedVersion).getOrElse('');
  }

  setSelectedVersionToAdd(version: string): void {
    this.addModPrefetch.ifSome(mi => mi.selectedVersion = version);
  }

  displayPrefetchTitle(): string {
    return this.addModPrefetch.map(mi => mi.title).getOrElse('');
  }

}
