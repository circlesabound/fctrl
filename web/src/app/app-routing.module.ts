import { NgModule } from '@angular/core';
import { RouterModule, Routes } from '@angular/router';
import { Dashboard2Component } from './dashboard2/dashboard2.component';
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
import { SystemComponent } from './logs/system/system.component';
import { ChatComponent } from './logs/chat/chat.component';
import { AuthGuard } from './auth/auth.guard';
import { OauthRedirectComponent } from './auth/oauth-redirect/oauth-redirect.component';
import { LoginComponent } from './auth/login/login.component';
import { DlcListComponent } from './mods/dlc-list/dlc-list.component';

const routes: Routes = [
  {
    path: 'login',
    component: LoginComponent,
    data: {
      title: 'fctrl | Login'
    }
  },
  {
    path: 'oauth-redirect',
    component: OauthRedirectComponent,
    data: {
      title: 'fctrl | Redirecting...'
    }
  },
  {
    path: 'dashboard',
    component: Dashboard2Component,
    canActivate: [AuthGuard],
    data: {
      title: 'fctrl | Dashboard',
    },
  },
  {
    path: 'server',
    component: ServerConfigComponent,
    canActivate: [AuthGuard],
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
    canActivate: [AuthGuard],
    children: [
      {
        path: '',
        redirectTo: 'list',
        pathMatch: 'full',
      },
      {
        path: 'dlc',
        component: DlcListComponent,
        data: {
          title: 'fctrl | DLC',
        }
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
    canActivate: [AuthGuard],
    children: [
      {
        path: '',
        redirectTo: 'system',
        pathMatch: 'full',
      },
      {
        path: 'system',
        component: SystemComponent,
        data: {
          title: 'fctrl | System Logs',
        },
      },
      {
        path: 'chat',
        component: ChatComponent,
        data: {
          title: 'fctrl | Chat Logs',
        },
      },
    ]
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
