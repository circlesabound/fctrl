import { Component, OnInit, OnDestroy } from '@angular/core';
import { Option } from 'prelude-ts';
import { Subscription } from 'rxjs';
import { webSocket } from 'rxjs/webSocket';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';

@Component({
  selector: 'app-chat',
  templateUrl: './chat.component.html',
  styleUrls: ['./chat.component.sass']
})
export class ChatComponent implements OnInit, OnDestroy {
  text = '';

  streamLogsSub: Option<Subscription>;
  monaco: any;
  autoscroll = true;

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
  }

  ngOnInit(): void {
    this.streamLogs();
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
        const ws = webSocket(location);
        ws.subscribe(
          async line => {
            this.text += ((line as any).content.ServerStdout + '\n');
            if (this.autoscroll) {
              const model = this.monaco.getModel();
              // this is sometimes null for whatever reason
              if (model !== null) {
                this.monaco.revealLine(model.getLineCount(), 1); // immediate scroll to bottom
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

  onEditorInit(editor: any): void {
    // editor is an ICodeEditor: https://microsoft.github.io/monaco-editor/api/interfaces/monaco.editor.icodeeditor.html
    this.monaco = editor;
  }

}
