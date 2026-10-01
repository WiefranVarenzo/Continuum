import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProjectLauncher } from "./ProjectLauncher";

describe("CP12 project launcher", () => {
  it("offers a recent project directly from Home", () => {
    const onOpen=vi.fn();
    render(<ProjectLauncher busy={false} recent={[{path:"/projects/continuum",name:"Continuum",research:true,development:true,openedAt:"2026-09-29T00:00:00Z"}]} onOpen={onOpen} onCreate={vi.fn()} onRestore={vi.fn()} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("button",{name:"Open Continuum"}));
    expect(onOpen).toHaveBeenCalledWith("/projects/continuum");
  });
  it("allows the folder name to stay empty and be replaced without restoring a default", () => {
    render(<ProjectLauncher busy={false} recent={[]} onOpen={vi.fn()} onCreate={vi.fn()} onRestore={vi.fn()} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "create" }));
    const folder = screen.getByLabelText(/New folder name/);
    expect(folder).toHaveValue("");
    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "Research" } });
    expect(folder).toHaveValue("");
    fireEvent.change(folder, { target: { value: "a" } });
    fireEvent.change(folder, { target: { value: "" } });
    expect(folder).toHaveValue("");
    fireEvent.change(folder, { target: { value: "Nama Saya" } });
    expect(folder).toHaveValue("Nama Saya");
  });
  it.each([
    ["Research only", "research"],
    ["Development only", "development"],
    ["Connected R&D", "connected"],
  ] as const)("uses the %s shortcut to open Create with the matching mode", (label, value) => {
    render(<ProjectLauncher busy={false} recent={[]} onOpen={vi.fn()} onCreate={vi.fn()} onRestore={vi.fn()} onError={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: label }));

    expect(screen.getByRole("heading", { name: "Create a new project" })).toBeInTheDocument();
    expect(screen.getByLabelText("Workspace mode")).toHaveValue(value);
  });

  it("creates a connected project with a safe derived folder name", async () => {
    const onCreate = vi.fn().mockResolvedValue(undefined);
    render(<ProjectLauncher busy={false} recent={[]} onOpen={vi.fn()} onCreate={onCreate} onRestore={vi.fn()} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "create" }));
    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "My First R&D" } });
    fireEvent.change(screen.getByLabelText("Parent folder"), { target: { value: "/projects" } });
    fireEvent.click(screen.getByRole("button", { name: "Create and open" }));
    expect(onCreate).toHaveBeenCalledWith({
      parentPath: "/projects",
      directoryName: "my-first-r-d",
      name: "My First R&D",
      research: true,
      development: true,
    });
  });

  it("keeps restore separate from opening an active project", () => {
    render(<ProjectLauncher busy={false} recent={[]} onOpen={vi.fn()} onCreate={vi.fn()} onRestore={vi.fn()} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "restore" }));
    expect(screen.getByRole("heading", { name: "Restore a verified export" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restore and verify" })).toBeDisabled();
  });

  it("restores a remote snapshot into a new local folder without opening over another project", () => {
    const onRestoreRemote = vi.fn().mockResolvedValue(undefined);
    render(<ProjectLauncher busy={false} recent={[]} onOpen={vi.fn()} onCreate={vi.fn()} onRestore={vi.fn()} onRestoreRemote={onRestoreRemote} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "from repo" }));
    fireEvent.change(screen.getByLabelText("Repository URL"), { target: { value: "https://github.com/wiefran/research.git" } });
    fireEvent.change(screen.getByLabelText("Restore into"), { target: { value: "/projects" } });
    fireEvent.change(screen.getByLabelText("New local folder name"), { target: { value: "research-copy" } });
    fireEvent.click(screen.getByRole("button", { name: "Download and verify as new project" }));
    expect(onRestoreRemote).toHaveBeenCalledWith({remoteUrl:"https://github.com/wiefran/research.git",destinationParent:"/projects",directoryName:"research-copy"});
  });

  it("explains missing import fields next to the button", async () => {
    const onRestoreRemote = vi.fn();
    render(<ProjectLauncher busy={false} recent={[]} onOpen={vi.fn()} onCreate={vi.fn()} onRestore={vi.fn()} onRestoreRemote={onRestoreRemote} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "from repo" }));
    fireEvent.click(screen.getByRole("button", { name: "Download and verify as new project" }));
    expect(await screen.findByText("Choose a GitHub repository or enter its URL first.")).toBeInTheDocument();
    expect(onRestoreRemote).not.toHaveBeenCalled();
  });

  it("shows a failed import beside the form instead of only at the top of Home", async () => {
    const onRestoreRemote = vi.fn().mockRejectedValue(new Error("Repository has no Continuum snapshot yet."));
    render(<ProjectLauncher busy={false} recent={[]} onOpen={vi.fn()} onCreate={vi.fn()} onRestore={vi.fn()} onRestoreRemote={onRestoreRemote} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "from repo" }));
    fireEvent.change(screen.getByLabelText("Repository URL"), { target: { value: "https://github.com/wiefran/research.git" } });
    fireEvent.change(screen.getByLabelText("Restore into"), { target: { value: "/projects" } });
    fireEvent.change(screen.getByLabelText("New local folder name"), { target: { value: "research-copy" } });
    fireEvent.click(screen.getByRole("button", { name: "Download and verify as new project" }));
    await waitFor(() => expect(screen.getByText("Repository has no Continuum snapshot yet.")).toBeInTheDocument());
  });
});
