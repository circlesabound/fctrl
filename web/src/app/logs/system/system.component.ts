import { Component, OnInit, OnDestroy } from '@angular/core';
import { Option } from 'prelude-ts';
import { Subscription } from 'rxjs';
import { webSocket } from 'rxjs/webSocket';
import { MgmtServerRestApiService } from 'src/app/mgmt-server-rest-api/services';

@Component({
  selector: 'app-system',
  templateUrl: './system.component.html',
  styleUrls: ['./system.component.sass']
})
export class SystemComponent implements OnInit, OnDestroy {
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
      category: 'systemlog'
    }).subscribe(response => {
      const location = response.headers.get('Location');

      if (location !== null) {
        const ws = webSocket(location);
        ws.subscribe(
          async line => {
            this.text += ((line as any).content.ServerStdout + '\n');
            if (this.autoscroll) {
              this.monaco.revealLine(this.monaco.getModel().getLineCount(), 1); // immediate scroll to bottom
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
