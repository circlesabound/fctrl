import { Component, OnInit, OnDestroy } from '@angular/core';
import { Option } from 'prelude-ts';
import { Subscription } from 'rxjs';
import { webSocket } from 'rxjs/webSocket';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';
import * as monaco from 'monaco-editor';
import { delay, tap } from 'rxjs/operators';
import { faHistory } from '@fortawesome/free-solid-svg-icons';

@Component({
  selector: 'app-chat',
  templateUrl: './chat.component.html',
  styleUrls: ['./chat.component.sass']
})
export class ChatComponent implements OnDestroy {
  text = '';

  streamLogsSub: Option<Subscription>;
  monaco!: monaco.editor.ICodeEditor;
  autoscroll = true;
  fetchPreviousMarker: Option<string>;

  fetchButtonIcon = faHistory;
  fetchButtonLoading = false;

  monacoOptions = {
    theme: 'vs-light',
    language: 'json',
    lineNumbers: 'off',
    readOnly: true,
  };

  constructor(
    private apiClient: MgmtServerRestApiService,
  ) {
    this.streamLogsSub = Option.none();
    this.fetchPreviousMarker = Option.none();
  }

  ngOnDestroy(): void {
    this.streamLogsSub.ifSome(s => s.unsubscribe());
  }

  streamLogs(): void {
    this.streamLogsSub = Option.some(this.apiClient.logsCategoryStreamGet$Response({
      category: 'chat'
    }).subscribe(response => {
      const location = response.headers.get('Location');

      if (location !== null) {
        if (response.body?.previous) {
          const from = response.body.previous;
          this.fetchPreviousMarker = Option.some(from);
        }

        const ws = webSocket(location);
        ws.subscribe(
          async line => {
            this.text += this.formatLine(line);
            if (this.autoscroll) {
              const model = this.monaco.getModel();
              // this is sometimes null for whatever reason
              if (model !== null) {
                this.monaco.revealLine(model.getLineCount(), monaco.editor.ScrollType.Immediate);
              }
            }
          },
          async err => {
            // TODO handle properly
            console.error(`ws error: ${err}`);
            ws.complete();
          },
          () => {
            // closing
          }
        );
      }
    }));
  }

  fetchPrevious(): void {
    this.fetchPreviousMarker.map(from => {
      this.fetchButtonLoading = true;
      this.apiClient.logsCategoryGet({
        category: 'chat',
        count: 50,
        direction: 'Backward',
        from: from,
      }).pipe(
        tap(previousLogs => {
          console.log('fetchPrevious returned');
          this.fetchButtonLoading = false;

          if (previousLogs.logs.length > 0) {
            let textToPrepend = '';
            const last = previousLogs.logs.length - 1;
            for (let i = last; i >= 0; --i) {
              const lineObj = JSON.parse(previousLogs.logs[i]);
              textToPrepend += this.formatLine(lineObj);
            }
            this.text = textToPrepend + this.text;
          }

          if (previousLogs.next) {
            console.log('Next fetchPrevious will start at ' + previousLogs.next);
            this.fetchPreviousMarker = Option.some(previousLogs.next);
          } else {
            console.log('No more results for fetchPrevious');
            this.fetchPreviousMarker = Option.none();
          }
        }),
        delay(100),
      ).subscribe(() => { });
    });
  }

  formatLine(logLineObject: any): string {
    const timestamp = logLineObject.timestamp;
    const logContent = logLineObject.content.ServerStdout;
    return '[' + timestamp + ']' + logContent + '\n';
  }

  onEditorInit(editor: monaco.editor.ICodeEditor): void {
    this.monaco = editor;
    this.streamLogs();
  }

}
