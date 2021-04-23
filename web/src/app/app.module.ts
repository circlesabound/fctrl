import { NgModule } from '@angular/core';
import { BrowserModule, Title } from '@angular/platform-browser';

import { AppRoutingModule } from './app-routing.module';
import { AppComponent } from './app.component';
import { DashboardComponent } from './dashboard/dashboard.component';
import { ServerConfigComponent } from './server-config/server-config.component';
import { ModsComponent } from './mods/mods.component';
import { LogsComponent } from './logs/logs.component';
import { PageNotFoundComponent } from './page-not-found/page-not-found.component';
import { MgmtServerRestApiModule } from './mgmt-server-rest-api/mgmt-server-rest-api.module';
import { HttpClientModule } from '@angular/common/http';
import { FontAwesomeModule } from '@fortawesome/angular-fontawesome';

@NgModule({
  declarations: [
    AppComponent,
    DashboardComponent,
    ServerConfigComponent,
    ModsComponent,
    LogsComponent,
    PageNotFoundComponent,
  ],
  imports: [
    BrowserModule,
    AppRoutingModule,
    HttpClientModule,
    MgmtServerRestApiModule.forRoot({ rootUrl: '/' }),
    FontAwesomeModule,
  ],
  providers: [
    Title,
  ],
  bootstrap: [
    AppComponent,
  ]
})
export class AppModule { }
