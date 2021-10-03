import { HttpEvent, HttpHandler, HttpInterceptor, HttpRequest } from '@angular/common/http';
import { Injectable } from '@angular/core';
import { Option } from 'prelude-ts';
import { Observable } from 'rxjs';

@Injectable()
export class BearerAuthInterceptor implements HttpInterceptor {
  constructor(private cfg: ApiRequestConfiguration) { }

  intercept(req: HttpRequest<any>, next: HttpHandler): Observable<HttpEvent<any>> {
    req = this.cfg.apply(req);

    return next.handle(req);
  }
}

@Injectable()
export class ApiRequestConfiguration {
  private nextAuthHeader: Option<string>;
  private nextAuthValue: Option<string>;

  constructor() {
    this.nextAuthHeader = Option.none();
    this.nextAuthValue = Option.none();
  }

  useBearerAuth(accessToken: string): void {
    this.nextAuthHeader = Option.some('Authorization');
    this.nextAuthValue = Option.some(`Bearer ${accessToken}`);
  }

  clear(): void {
    this.nextAuthHeader = Option.none();
    this.nextAuthValue = Option.none();
  }

  apply(req: HttpRequest<any>): HttpRequest<any> {
    const headers: {[k: string]: string} = {};
    this.nextAuthHeader.ifSome(h => {
      this.nextAuthValue.ifSome(v => {
        headers[h] = v;
      });
    });
    return req.clone({
      setHeaders: headers,
    });
  }
}
