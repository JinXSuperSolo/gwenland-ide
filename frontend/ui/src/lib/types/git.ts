export type CommitEdgeKind = 'linear' | 'fork' | 'merge'

export interface CommitNode {
  hash: string
  shortHash: string
  message: string
  author: string
  date: string
  relativeDate: string
  parents: string[]
  refs: string[]
  lane: number
  x: number
  y: number
  isHead: boolean
  isMerge: boolean
}

export interface CommitEdge {
  from: string
  to: string
  fromLane: number
  toLane: number
  fromX: number
  toX: number
  kind: CommitEdgeKind
}

export interface BranchRef {
  name: string
  hash: string
  isRemote: boolean
  lane: number
}

export interface CommitGraphPayload {
  nodes: CommitNode[]
  edges: CommitEdge[]
  branches: BranchRef[]
  head: string | null
  truncated: boolean
}

export interface CommitFileChange {
  path: string
  status: string
}

export interface CommitDetails {
  hash: string
  fullMessage: string
  author: string
  date: string
  filesChanged: CommitFileChange[]
  insertions: number
  deletions: number
}
