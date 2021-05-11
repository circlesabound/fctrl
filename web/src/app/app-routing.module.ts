import { NgModule } from '@angular/core';
import { RouterModule, Routes } from '@angular/router';
import { DashboardComponent } from './dashboard/dashboard.component';
import { ServerConfigComponent } from './server-config/server-config.component';
import { ModsComponent } from './mods/mods.component';
import { LogsComponent } from './logs/logs.component';
import { PageNotFoundComponent } from './page-not-found/page-not-found.component';
import { ModListComponent } from './mods/mod-list/mod-list.component';
import { ModSettingsComponent } from './mods/mod-settings/mod-settings.component';
import { AdminListComponent } from './server-config/admin-list/admin-list.component';
import { ServerSettingsComponent } from './server-config/server-settings/server-settings.component';
import { BanListComponent } from './server-config/ban-list/ban-list.component';
import { WhiteListComponent } from './server-config/white-list/white-list.component';
import { SecretsComponent } from './server-config/secrets/secrets.component';

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
    children: [
      {
        path: '',
        redirectTo: 'server-settings',
        pathMatch: 'full',
      },
      {
        path: 'admin-list',
        component: AdminListComponent,
        data: {
          title: 'fctrl | Admin List',
        },
      },
      {
        path: 'ban-list',
        component: BanListComponent,
        data: {
          title: 'fctrl | Ban List',
        },
      },
      {
        path: 'secrets',
        component: SecretsComponent,
        data: {
          title: 'fctrl | Secrets',
        },
      },
      {
        path: 'server-settings',
        component: ServerSettingsComponent,
        data: {
          title: 'fctrl | Server Settings',
        },
      },
      {
        path: 'white-list',
        component: WhiteListComponent,
        data: {
          title: 'fctrl | White List',
        },
      },
    ],
    data: {
      title: 'fctrl | Config',
    },
  },
  {
    path: 'mods',
    component: ModsComponent,
    children: [
      {
        path: '',
        redirectTo: 'list',
        pathMatch: 'full',
      },
      {
        path: 'list',
        component: ModListComponent,
        data: {
          title: 'fctrl | Mod List',
        },
      },
      {
        path: 'settings',
        component: ModSettingsComponent,
        data: {
          title: 'fctrl | Mod Settings',
        }
      },
    ],
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
