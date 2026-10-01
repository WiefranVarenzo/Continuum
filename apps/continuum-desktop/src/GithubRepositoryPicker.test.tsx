import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GithubRepositoryPicker } from "./GithubRepositoryPicker";

const mocks = vi.hoisted(() => ({
  getGithubStatus: vi.fn(),
  listGithubRepositories: vi.fn(),
  listGithubBranches: vi.fn(),
  beginGithubLogin: vi.fn(),
  getGithubLoginProgress: vi.fn(),
  openGithubAuthorizationPage: vi.fn(),
  openGithubNewRepository: vi.fn(),
}));

vi.mock("./bridge", () => ({
  ...mocks,
  isDesktopRuntime: () => true,
}));

describe("GitHub repository picker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.openGithubAuthorizationPage.mockResolvedValue(undefined);
    mocks.getGithubStatus.mockResolvedValue({ cli_available: true, connected: true, username: "wiefran" });
    mocks.listGithubRepositories.mockResolvedValue([{ full_name: "wiefran/research", clone_url: "https://github.com/wiefran/research.git", default_branch: "main", private: true }]);
    mocks.listGithubBranches.mockResolvedValue(["main", "review"]);
  });

  it("chooses a private repository and a branch without uploading", async () => {
    const onSelect = vi.fn();
    const view = render(<GithubRepositoryPicker remoteUrl="" branch="" onSelect={onSelect} />);
    await screen.findByText("Connected as @wiefran");
    fireEvent.change(await screen.findByLabelText("Choose a private GitHub repository"), { target: { value: "https://github.com/wiefran/research.git" } });
    expect(onSelect).toHaveBeenCalledWith("https://github.com/wiefran/research.git", "main");

    view.rerender(<GithubRepositoryPicker remoteUrl="https://github.com/wiefran/research.git" branch="main" onSelect={onSelect} />);
    await waitFor(() => expect(screen.getByLabelText("Branch").querySelectorAll("option")).toHaveLength(2));
    fireEvent.change(screen.getByLabelText("Branch"), { target: { value: "review" } });
    expect(onSelect).toHaveBeenCalledWith("https://github.com/wiefran/research.git", "review");
    expect(mocks.beginGithubLogin).not.toHaveBeenCalled();
  });

  it("makes login an explicit user action", async () => {
    mocks.getGithubStatus.mockResolvedValue({ cli_available: true, connected: false, username: null });
    mocks.listGithubRepositories.mockResolvedValue([]);
    mocks.beginGithubLogin.mockResolvedValue({ phase: "waiting", user_code: null, message: null });
    render(<GithubRepositoryPicker remoteUrl="" branch="" onSelect={vi.fn()} />);
    await screen.findByRole("button", { name: "Login with GitHub" });
    expect(mocks.beginGithubLogin).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Login with GitHub" }));
    await waitFor(() => expect(mocks.beginGithubLogin).toHaveBeenCalledOnce());
    fireEvent.click(await screen.findByRole("button", { name: "Open GitHub authorization page" }));
    await waitFor(() => expect(mocks.openGithubAuthorizationPage).toHaveBeenCalledOnce());
  });
});
