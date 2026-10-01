import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ReleaseCenter } from "./ReleaseCenter";

describe("CP12 release center", () => {
  it("shows project health and release recovery actions", () => {
    render(<ReleaseCenter
      project={{ project_id: "p1", name: "Continuum pilot", path: "/projects/pilot", status: "active", ledger_sequence: 42, research: true, development: true, integrity_healthy: true }}
      diagnostics={{ app_version: "0.12.0", schema_version: 12, project_id: "p1", project_name: "Continuum pilot", project_path: "/projects/pilot", project_status: "active", ledger_sequence: 42, research_enabled: true, development_enabled: true, checked_artifacts: 8, integrity_healthy: true, issues: [] }}
      busy={false}
      onDiagnose={vi.fn()}
      onBackup={vi.fn()}
      onExport={vi.fn()}
      onClose={vi.fn()}
    />);
    expect(screen.getByText("All integrity checks passed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Backup database" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Export restorable copy" })).toBeEnabled();
  });
});
