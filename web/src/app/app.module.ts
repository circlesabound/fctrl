import { HttpClientModule } from '@angular/common/http';
import { NgModule } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { BrowserModule, Title } from '@angular/platform-browser';

import { AppRoutingModule } from './app-routing.module';
import { AppComponent } from './app.component';
import { DashboardComponent } from './dashboard/dashboard.component';
import { ServerConfigComponent } from './server-config/server-config.component';
import { ModsComponent } from './mods/mods.component';
import { LogsComponent } from './logs/logs.component';
import { PageNotFoundComponent } from './page-not-found/page-not-found.component';
import { MgmtServerRestApiModule } from './mgmt-server-rest-api/mgmt-server-rest-api.module';
import { FontAwesomeModule } from '@fortawesome/angular-fontawesome';
import { ModListComponent } from './mods/mod-list/mod-list.component';
import { ModSettingsComponent } from './mods/mod-settings/mod-settings.component';
import { AdminListComponent } from './server-config/admin-list/admin-list.component';
import { SecretsComponent } from './server-config/secrets/secrets.component';
import { ServerSettingsComponent } from './server-config/server-settings/server-settings.component';
import { BanListComponent } from './server-config/ban-list/ban-list.component';
import { WhiteListComponent } from './server-config/white-list/white-list.component';
import { NotificationsComponent } from './notifications/notifications.component';
import { EditableListComponent } from './editable-list/editable-list.component';
import { MonacoEditorModule } from 'ngx-monaco-editor';
import { ModObjectComponent } from './mods/mod-list/mod-object/mod-object.component';
import { FactorioModPortalApiModule } from './factorio-mod-portal-api/factorio-mod-portal-api.module';
import { environment } from 'src/environments/environment';

@NgModule({
  declarations: [
    AppComponent,
    DashboardComponent,
    ServerConfigComponent,
    ModsComponent,
    LogsComponent,
    PageNotFoundComponent,
    ModListComponent,
    ModSettingsComponent,
    AdminListComponent,
    SecretsComponent,
    ServerSettingsComponent,
    BanListComponent,
    WhiteListComponent,
    NotificationsComponent,
    EditableListComponent,
    ModObjectComponent,
  ],
  imports: [
    BrowserModule,
    AppRoutingModule,
    FormsModule,
    HttpClientModule,
    MgmtServerRestApiModule.forRoot({ rootUrl: `http://${environment.backendHost}/api/v0` }),
    FactorioModPortalApiModule.forRoot({ rootUrl: `http://${environment.backendHost}/proxy` }),
    FontAwesomeModule,
    MonacoEditorModule.forRoot({
      defaultOptions: {
        fixedOverflowWidgets: true,
        minimap: {
          enabled: false,
        },
      }
    }),
  ],
  providers: [
    Title,
  ],
  bootstrap: [
    AppComponent,
  ]
})
export class AppModule { }
