import { Component, OnInit } from '@angular/core';
import { faAngleDown, faCheck, faPlus, faSave } from '@fortawesome/free-solid-svg-icons';
import { Option } from 'prelude-ts';
import { Observable, Subject } from 'rxjs';
import { debounceTime, delay, distinctUntilChanged, map, switchMap, tap } from 'rxjs/operators';
import { FactorioModPortalApiService } from 'src/app/factorio-mod-portal-api/services';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';
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
  dropdownArrowIcon = faAngleDown;
  addIcon = faPlus;
  saveIcon = faSave;
  tickIcon = faCheck;

  constructor(
    private apiClient: MgmtServerRestApiService,
    private modPortalClient: FactorioModPortalApiService,
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
            versions: remoteInfo.releases?.map(r => r.version) ?? [],
          });
        }
        this.modInfoList = infoList;
      });
    });
  }

  pushModList(): void {
    this.saveButtonLoading = true;

    this.apiClient.serverModsListPost({
      body: this.modInfoList.map(info => {
        return {
          name: info.name,
          version: info.selectedVersion,
        };
      }),
    }).pipe(
      tap(() => {
        console.log('pushModList returned');
        this.saveButtonLoading = false;
        this.showTickIcon = true;
      }),
      delay(3000),
    ).subscribe(() => {
      this.showTickIcon = false;
    });
  }

  addMod(): void {
    this.addModPrefetch.ifSome(info => {
      this.modInfoList.push(info);
    });
    this.addModPrefetch = Option.none();
    this.addModName = '';
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
      map(infoShort => {
        console.log(`prefetched mod '${infoShort.name}'`);
        const versions = infoShort.releases.map(r => r.version);
        const ret: ModInfo = {
          name: infoShort.name,
          title: infoShort.title,
          summary: infoShort.summary,
          versions,
          selectedVersion: versions[versions.length - 1],
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
