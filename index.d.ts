/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export class Storage {
  constructor(path: string)
  error(): string | null
  connect(workspaceId: string, remote: string): Workspace | null
  sync(workspaceId: string, remote: string): Workspace
}
export class Workspace {
  constructor(id: string)
  id(): string
  clientId(): number
  static search(self: Workspace, query: string): string
  static getSearchIndex(self: Workspace): Array<string>
  static setSearchIndex(self: Workspace, fields: Array<string>): boolean
}
