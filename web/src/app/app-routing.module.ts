import { NgModule } from '@angular/core';
import { RouterModule, Routes } from '@angular/router';
import { DashboardComponent } from './dashboard/dashboard.component';
import { ServerConfigComponent } from './server-config/server-config.component';
import { ModsComponent } from './mods/mods.component';
import { LogsComponent } from './logs/logs.component';
import { PageNotFoundComponent } from './page-not-found/page-not-found.component';

const routes: Routes = [
  {
    path: 'dashboard',
    component: DashboardComponent,
    data: {
      title: 'fctrl | Dashboard',
    },
  },
  {
    path: 'server',
    component: ServerConfigComponent,
    data: {
      title: 'fctrl | Config',
    },
  },
  {
    path: 'mods',
    component: ModsComponent,
    data: {
      title: 'fctrl | Mods',
    },
  },
  {
    path: 'logs',
    component: LogsComponent,
    data: {
      title: 'fctrl | Logs',
    },
  },
  {
    path: '',
    redirectTo: 'dashboard',
    pathMatch: 'full',
  },
  {
    path: '**',
    component: PageNotFoundComponent,
    data: {
      title: 'fctrl | 404',
    },
  },
];

@NgModule({
  imports: [RouterModule.forRoot(routes)],
  exports: [RouterModule]
})
export class AppRoutingModule { }
