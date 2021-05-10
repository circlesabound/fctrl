import { Component, OnInit, Output, EventEmitter, Input, OnDestroy } from '@angular/core';
import { faHdd } from '@fortawesome/free-regular-svg-icons';
import { Option } from 'prelude-ts';
import { Observable, of, Subscription, timer } from 'rxjs';
import { catchError } from 'rxjs/operators';
import { ServerControlStatus } from 'src/app/mgmt-server-rest-api/models';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';
import { StatusControl } from '../status-control';

@Component({
  selector: 'app-status',
  templateUrl: './status.component.html',
  styleUrls: ['./status.component.sass']
})
export class StatusComponent implements OnInit, OnDestroy {
  @Input() triggerUpdateEvents!: Observable<void>;
  @Output() statusControlEvent = new EventEmitter<StatusControl>();

  @Output() versionEvent = new EventEmitter<string>();

  status: Option<ServerControlStatus>;
  manualUpdateStatusSubscription: Option<Subscription>;

  version: Option<string>;

  periodicUpdateAll: Option<Subscription>;

  statusIcon = faHdd;

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) {
    this.status = Option.none();
    this.manualUpdateStatusSubscription = Option.none();

    this.version = Option.none();

    this.periodicUpdateAll = Option.none();
  }

  ngOnInit(): void {
    this.manualUpdateStatusSubscription = Option.some(this.triggerUpdateEvents.subscribe(() => {
      this.updateStatus();
    }));

    this.periodicUpdateAll = Option.some(timer(0, 5000).subscribe(() => {
      this.updateStatus();
      this.updateVersion();
    }));
  }

  ngOnDestroy(): void {
    this.periodicUpdateAll.map(s => s.unsubscribe());
    this.manualUpdateStatusSubscription.map(s => s.unsubscribe());
  }

  getStatusIconClass(): string {
    return this.status.map(s => {
      if (s.game_status === 'InGame') {
        return 'has-text-success';
      } else if (s.game_status === 'PreGame' || s.game_status === 'PostGame') {
        return 'has-text-warning';
      } else {
        return 'has-text-danger';
      }
    }).getOrElse('has-text-danger');
  }

  getStatusText(): string {
    return this.status.map(s => {
      if (s.game_status === 'InGame') {
        return 'Online';
      } else if (s.game_status === 'PreGame') {
        return 'Starting';
      } else if (s.game_status === 'PostGame') {
        return 'Stopping';
      } else if (s.game_status === 'NotRunning') {
        return 'Offline';
      } else {
        return 'Unknown';
      }
    }).getOrElse('Unknown');
  }

  getPlayersText(): string {
    return this.status.map(s => s.player_count.toString()).getOrElse('N/A');
  }

  getVersionText(): string {
    return this.version.getOrElse('N/A');
  }

  private updateStatus(): void {
    this.apiClient.serverControlGet().subscribe(s => {
      const prevOpt = this.status;
      this.status = Option.some(s);

      this.status.map(curr => {
        if (prevOpt.map(prev => prev.game_status !== curr.game_status).getOrElse(true)) {
          let valueToEmit = StatusControl.Invalid;
          if (curr.game_status === 'NotRunning') {
            valueToEmit = StatusControl.CanStart;
          } else if (curr.game_status === 'PreGame') {
            valueToEmit = StatusControl.Starting;
          } else if (curr.game_status === 'InGame') {
            valueToEmit = StatusControl.CanStop;
          } else if (curr.game_status === 'PostGame') {
            valueToEmit = StatusControl.Stopping;
          }
          this.statusControlEvent.emit(valueToEmit);
        }
      });
    });
  }

  private updateVersion(): void {
    this.apiClient.serverInstallGet().pipe(
      catchError(e => {
        // error means not installed
        return of({
          version: 'not installed'
        });
      })
    ).subscribe(v => {
      const prevOpt = this.version;
      this.version = Option.some(v.version);

      if (prevOpt.map(prev => prev !== v.version).getOrElse(true)) {
        this.versionEvent.emit(v.version);
      }
    });
  }

}
