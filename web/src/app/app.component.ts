import { Component, OnInit } from '@angular/core';
import { Title } from '@angular/platform-browser';
import { ActivatedRoute, NavigationEnd, Router } from '@angular/router';
import { faChartBar, faCogs, faTerminal, faWrench } from '@fortawesome/free-solid-svg-icons';
import { filter, map, mergeMap } from 'rxjs/operators';

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

  constructor(
    private router: Router,
    private activatedRoute: ActivatedRoute,
    private title: Title) { }

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
  }

  public onBurgerClick(): void {
    this.navBurgerExpanded = !this.navBurgerExpanded;
  }

  public onNavAway(): void {
    this.navBurgerExpanded = false;
  }
}
