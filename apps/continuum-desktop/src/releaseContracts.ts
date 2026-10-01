export interface OpenedProject {
  project_id: string;
  name: string;
  path: string;
  status: string;
  ledger_sequence: number;
  research: boolean;
  development: boolean;
  integrity_healthy: boolean;
}

export interface IntegrityIssue {
  code: string;
  path_or_id: string;
  guidance: string;
}

export interface ReleaseDiagnostics {
  app_version: string;
  schema_version: number;
  project_id: string;
  project_name: string;
  project_path: string;
  project_status: string;
  ledger_sequence: number;
  research_enabled: boolean;
  development_enabled: boolean;
  checked_artifacts: number;
  integrity_healthy: boolean;
  issues: IntegrityIssue[];
}

export interface CreateProjectInput {
  parentPath: string;
  directoryName: string;
  name: string;
  research: boolean;
  development: boolean;
}

export interface RestoreProjectInput {
  sourcePath: string;
  destinationParent: string;
  directoryName: string;
}

export interface RestoreRemoteInput {
  remoteUrl: string;
  branch?: string;
  destinationParent: string;
  directoryName: string;
}

export interface ProjectRemoteStatus {
  remote_url: string | null;
  branch: string | null;
  last_head: string | null;
  git_available: boolean;
  lfs_available: boolean;
  estimated_current_bytes: number;
}

export interface ProjectPublishResult {
  remote_url: string;
  commit_id: string;
  changed: boolean;
}

export interface GithubStatus {
  cli_available: boolean;
  connected: boolean;
  username: string | null;
}

export interface GithubLoginProgress {
  phase: "" | "idle" | "waiting" | "connected" | "error";
  user_code: string | null;
  message: string | null;
}

export interface GithubRepository {
  full_name: string;
  clone_url: string;
  default_branch: string;
  private: boolean;
}

export interface WorkspaceQuestionInput {
  title: string;
  question: string;
  context: string;
  desiredOutcome: string;
  priority: number;
}

export interface RepositorySyncSummary {
  repository_id: string;
  baseline_id: string;
  branch_name: string | null;
  head_oid: string | null;
  worktree_changes: number;
  ingested_commits: number;
  remaining_commits: number;
  analyzed_files: number;
  code_entities: number;
  discovered_tests: number;
  analysis_completeness: string;
}
