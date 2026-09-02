# Continuum Core Architecture Diagrams

> **Status:** Approved CP1 Mermaid sources. Written architecture remains authoritative when rendering support differs.

## 1. Big Picture

```mermaid
flowchart TB
  U[User] --> UI[Continuum Desktop]
  UI --> RC[Research Space]
  UI --> DC[Development Space]
  RC <-. optional R&D Bridge .-> DC
  RC --> CC[Continuity Core]
  DC --> CC
  CC --> DB[(SQLite Ledger)]
  CC --> AS[(Artifact Store)]
  CC --> CE[Checkpoint & Context Engine]
  CC --> PG[Privacy Gateway]
  PG --> GM[Gemini]
```

## 2. System Context

```mermaid
flowchart LR
  User --> Continuum
  Continuum <--> Repo[Local Git Repository]
  Continuum <--> Files[Files / Web Evidence / Capture]
  Continuum --> Gemini[Gemini API]
  Client[Permissioned AI Client] <--> Continuum
  Continuum --> Export[Portable Project Export]
```

## 3. Research-only

```mermaid
flowchart TD
  Q[Research Question] --> E[Evidence]
  Q --> X[Experiment]
  X --> R[Result]
  E --> F[Finding]
  R --> F
  F --> D[Decision]
  Q -. any meaningful point .-> C[Research Checkpoint]
  E -.-> C
  X -.-> C
  F -.-> C
```

## 4. Development-only

```mermaid
flowchart TD
  O[Manual / External / Legacy Intent] --> R[Requirement or ChangeSet]
  R --> C[CodeEntity Changes]
  C --> T[Test]
  T --> TR[TestRun / Validation]
  R -. any meaningful point .-> CP[Development Checkpoint]
  C -.-> CP
  TR -.-> CP
```

## 5. Connected R&D Loop

```mermaid
flowchart LR
  E[Evidence / Result] --> F[Finding]
  F --> D[Decision]
  D --> R[Requirement]
  R --> CS[ChangeSet]
  CS --> C[Code]
  C --> T[Test / TestRun]
  T --> V{Validation}
  V -->|expected| CP[Integrated Checkpoint]
  V -->|new learning| LF[Learning Feedback]
  LF -. review .-> E
  LF -. revision .-> F
  LF -. revision .-> R
```

## 6. Component Architecture

```mermaid
flowchart TB
  React[React UI] --> IPC[Typed Tauri IPC]
  IPC --> APP[Application Services]
  APP --> CORE[Continuity Domain]
  APP --> RS[Research Domain]
  APP --> DS[Development Domain]
  APP --> BR[Bridge Policies]
  CORE --> PORTS[Ports]
  RS --> PORTS
  DS --> PORTS
  PORTS --> SQL[SQLite Adapter]
  PORTS --> ART[Artifact Adapter]
  PORTS --> GIT[Git / Analyzer Adapters]
  PORTS --> AI[AI Gateway]
  PORTS --> CAP[Capture Adapter]
```

## 7. Deterministic and Semantic Boundary

```mermaid
flowchart LR
  S[Canonical Sources] --> R[Deterministic Retrieval]
  R --> P[Privacy Filter]
  P --> G[Gemini Semantic Task]
  G --> SV[Schema + Source Validation]
  SV --> C[Pending Candidate]
  C --> H{Human Review}
  H -->|accept/edit| CMD[Canonical Command]
  H -->|reject| X[Retain Rejection Metadata]
```

## 8. AI Privacy Flow

```mermaid
flowchart TD
  Req[AI Task] --> Resolve[Resolve Direct + Transitive Sources]
  Resolve --> Classify[Classify + Secret Scan]
  Classify --> Deny{Denied?}
  Deny -->|yes| Remove[Exclude + Record Omission]
  Deny -->|no| Consent{Consent Required?}
  Consent -->|yes| Preview[User Preview]
  Consent -->|no| Send[Bounded Provider Request]
  Preview --> Send
  Send --> Validate[Validate Response]
```

## 9. Checkpoint Resume

```mermaid
sequenceDiagram
  participant U as User
  participant C as Continuity Core
  participant L as Ledger
  participant E as Context Engine
  U->>C: Create checkpoint(scope)
  C->>L: Read consistent state + sequence
  C->>L: Commit immutable checkpoint
  U->>E: Resume later
  E->>L: Load checkpoint + later events
  E-->>U: Then / Since / Now / Next
```

## 10. Context Pack

```mermaid
flowchart LR
  Task --> Tier1[Checkpoint + Goals + Next]
  Tier1 --> Tier2[Direct Entities + 1-hop Graph]
  Tier2 --> Tier3[Evidence / Diff / Tests]
  Tier3 --> Budget{Budget Remaining?}
  Budget -->|yes| Tier4[Requested Deep Artifacts]
  Budget -->|no| Pack[Pack + Omission Summary]
  Tier4 --> Pack
  Pack --> Privacy[Final Privacy Gate]
```
