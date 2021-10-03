import { Injectable } from '@angular/core';
import { Option } from 'prelude-ts';
import { Observable } from 'rxjs';
import { map, tap } from 'rxjs/operators';
import { MgmtServerRestApiService } from '../mgmt-server-rest-api/services';

@Injectable({
  providedIn: 'root'
})
export class AuthDiscordService {
  private accessToken: Option<string>;
  private expiry: Option<Date>;

  constructor(private apiClient: MgmtServerRestApiService) {
    this.accessToken = Option.none();
    this.expiry = Option.none();
  }

  private getStoredDataIfValid(): Option<OAuthStoredData> {
    let storedDataStr = localStorage.getItem(LOCAL_STORAGE_OAUTH_STORED_DATA_KEY);
    if (storedDataStr) {
      let storedData: OAuthStoredData = JSON.parse(storedDataStr);
      if (storedData.expiry < new Date()) {
        // expired
        console.log('oauth data from localStorage is expired');
        localStorage.removeItem(LOCAL_STORAGE_OAUTH_STORED_DATA_KEY);
        return Option.none();
      }
      return Option.some(storedData);
    } else {
      return Option.none();
    }
  }

  getAuthorisationUrl(client_id: string): string {
    return `${DISCORD_AUTHORISATION_ENDPOINT}?response_type=code&client_id=${client_id}&scope=identify&prompt=none&redirect_uri=${window.location.origin}/oauth-redirect`;
  }

  tryGetAccessToken(): Option<string> {
    if (this.accessToken.isSome()) {
      return this.accessToken;
    } else {
      return this.getStoredDataIfValid().map(storedData => {
        console.log('read oauth data from localStorage');
        this.accessToken = Option.some(storedData.access_token);
        this.expiry = Option.some(storedData.expiry);
        return storedData.access_token;
      });
    }
  }

  codeToToken(accessCode: string): Observable<string> {
    return this.apiClient.authDiscordGrantPost({ code: accessCode }).pipe(
      tap(s => {
        var expiry = new Date();
        if (s.expires_in) {
          expiry.setDate(expiry.getSeconds() + s.expires_in);
        } else {
          expiry.setFullYear(9999, 1, 1);
        }
        this.accessToken = Option.some(s.access_token);
        let storedData: OAuthStoredData = {
          access_token: s.access_token,
          expiry,
        };
        console.log('storing oauth data in localStorage');
        localStorage.setItem(LOCAL_STORAGE_OAUTH_STORED_DATA_KEY, JSON.stringify(storedData));
      }),
      map(s => s.access_token),
    );
  }

  private refresh(): OAuthStoredData {
    // TODO
    return {
      access_token: 'a',
      expiry: new Date(),
    };
  }

  logout(): void {
    localStorage.removeItem(LOCAL_STORAGE_OAUTH_STORED_DATA_KEY);
    this.accessToken = Option.none();
  }
}

const DISCORD_AUTHORISATION_ENDPOINT: string = 'https://discord.com/api/oauth2/authorize';
const LOCAL_STORAGE_OAUTH_STORED_DATA_KEY: string = 'fctrlDiscordOAuth';

interface OAuthStoredData {
  access_token: string;
  expiry: Date;
}
