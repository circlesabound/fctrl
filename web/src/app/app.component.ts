import { Component, OnInit } from '@angular/core';
import { Title } from '@angular/platform-browser';
import { ActivatedRoute, NavigationEnd, Router } from '@angular/router';
import { faChartBar, faCogs, faTerminal, faWrench } from '@fortawesome/free-solid-svg-icons';
import { filter, map, mergeMap } from 'rxjs/operators';
import { OperationService } from './operation.service';
import { MgmtServerRestApiService } from './mgmt-server-rest-api/services';
import { BuildVersion } from './mgmt-server-rest-api/models';

@Component({
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.sass']
})
export class AppComponent implements OnInit {
  navBurgerExpanded = false;

  navDashboardIcon = faChartBar;
  navConfigIcon = faCogs;
  navModsIcon = faWrench;
  navLogsIcon = faTerminal;

  agentBuildInfo: BuildVersion | undefined = undefined;
  mgmtServerBuildInfo: BuildVersion | undefined = undefined;

  constructor(
    private router: Router,
    private activatedRoute: ActivatedRoute,
    private title: Title,
    private operationService: OperationService,
    private apiClient: MgmtServerRestApiService,
  ) { }

  ngOnInit(): void {
    this.router.events.pipe(
      filter(event => event instanceof NavigationEnd),
      map(() => this.activatedRoute),
      map((route) => {
        while (route.firstChild) {
          route = route.firstChild;
        }

        return route;
      }),
      mergeMap((route) => route.data)
    ).subscribe((event) => this.title.setTitle(event.title));

    this.apiClient.buildinfoGet().subscribe(bi => {
      this.agentBuildInfo = bi.agent;
      this.mgmtServerBuildInfo = bi.mgmt_server;
    });
  }

  public onBurgerClick(): void {
    this.navBurgerExpanded = !this.navBurgerExpanded;
  }

  public onNavAway(): void {
    this.navBurgerExpanded = false;
  }
}
