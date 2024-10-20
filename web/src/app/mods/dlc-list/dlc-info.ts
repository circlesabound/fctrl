import { DlcName } from "src/app/mgmt-server-rest-api/models";

export interface DlcInfo {
  name: DlcName,
  title: string,
  enabled: boolean,
}
