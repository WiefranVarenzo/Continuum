import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceQuickStart } from "./WorkspaceQuickStart";

describe("CP12 workspace quick start", () => {
  it("submits a real research question through the canonical workflow", async () => {
    const onQuestion = vi.fn().mockResolvedValue(undefined);
    render(<WorkspaceQuickStart research development={false} busy={false} onQuestion={onQuestion} onRepository={vi.fn()} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Short title"), { target: { value: "Resume quality" } });
    fireEvent.change(screen.getByLabelText("Question"), { target: { value: "Can a user resume in two minutes?" } });
    fireEvent.change(screen.getByLabelText("Context"), { target: { value: "Pilot readiness" } });
    fireEvent.change(screen.getByLabelText("Desired outcome"), { target: { value: "A measured answer" } });
    fireEvent.click(screen.getByRole("button", { name: "Save question" }));
    await waitFor(() => expect(onQuestion).toHaveBeenCalledWith({
      title: "Resume quality",
      question: "Can a user resume in two minutes?",
      context: "Pilot readiness",
      desiredOutcome: "A measured answer",
      priority: 2,
    }));
  });

  it("keeps Development setup independent and reports bounded analysis", async () => {
    const onRepository = vi.fn().mockResolvedValue({
      repository_id: "repo-1", baseline_id: "base-1", branch_name: "main", head_oid: "abc",
      worktree_changes: 0, ingested_commits: 4, remaining_commits: 0, analyzed_files: 12,
      code_entities: 38, discovered_tests: 6, analysis_completeness: "complete",
    });
    render(<WorkspaceQuickStart research={false} development busy={false} onQuestion={vi.fn()} onRepository={onRepository} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Local Git folder"), { target: { value: "/projects/repository" } });
    fireEvent.click(screen.getByRole("button", { name: "Analyze local folder" }));
    await waitFor(() => expect(onRepository).toHaveBeenCalledWith("/projects/repository"));
    expect(await screen.findByText("12 files · 38 code entities · 6 tests")).toBeInTheDocument();
  });
});
