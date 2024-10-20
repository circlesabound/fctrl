import { HttpClientModule, HTTP_INTERCEPTORS } from '@angular/common/http';
import { forwardRef, NgModule, Provider } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { BrowserModule, Title } from '@angular/platform-browser';
import { BrowserAnimationsModule } from '@angular/platform-browser/animations'
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { NgxMatFileInputModule } from '@angular-material-components/file-input';

import { AppRoutingModule } from './app-routing.module';
import { AppComponent } from './app.component';
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
import { MonacoEditorModule } from 'ngx-monaco-editor-v2';
import { ModObjectComponent } from './mods/mod-list/mod-object/mod-object.component';
import { FactorioModPortalApiModule } from './factorio-mod-portal-api/factorio-mod-portal-api.module';
import { environment } from 'src/environments/environment';
import { ClickOutsideModule } from 'ng-click-outside';
import { ChatComponent } from './logs/chat/chat.component';
import { SystemComponent } from './logs/system/system.component';
import { Dashboard2Component } from './dashboard2/dashboard2.component';
import { ApiRequestConfiguration, BearerAuthInterceptor } from './auth/bearer-auth-interceptor';
import { OauthRedirectComponent } from './auth/oauth-redirect/oauth-redirect.component';
import { LoginComponent } from './auth/login/login.component';
import { LogoutComponent } from './auth/logout/logout.component';
import { DlcListComponent } from './mods/dlc-list/dlc-list.component';

export const BEARER_AUTH_INTERCEPTOR_PROVIDER: Provider = {
  provide: HTTP_INTERCEPTORS,
  useExisting: forwardRef(() => BearerAuthInterceptor),
  multi: true,
};

@NgModule({
  declarations: [
    AppComponent,
    ServerConfigComponent,
    ModsComponent,
    LogsComponent,
    PageNotFoundComponent,
    DlcListComponent,
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
    ChatComponent,
    SystemComponent,
    Dashboard2Component,
    OauthRedirectComponent,
    LoginComponent,
    LogoutComponent,
  ],
  imports: [
    BrowserModule,
    BrowserAnimationsModule,
    AppRoutingModule,
    FormsModule,
    HttpClientModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    MgmtServerRestApiModule.forRoot({ rootUrl: `${window.location.protocol}//${environment.backendHost}/api/v0` }),
    FactorioModPortalApiModule.forRoot({ rootUrl: `${window.location.protocol}//${environment.backendHost}/proxy` }),
    FontAwesomeModule,
    MonacoEditorModule.forRoot({
      defaultOptions: {
        fixedOverflowWidgets: true,
        minimap: {
          enabled: false,
        },
      }
    }),
    NgxMatFileInputModule,
    ClickOutsideModule,
  ],
  providers: [
    Title,
    ApiRequestConfiguration,
    BearerAuthInterceptor,
    BEARER_AUTH_INTERCEPTOR_PROVIDER,
  ],
  bootstrap: [
    AppComponent,
  ]
})
export class AppModule { }
