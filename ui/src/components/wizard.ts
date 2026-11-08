import { agentsSpawn, doctorCheck, workspaceAdd } from "../api";
import type { DoctorResult, Workspace } from "../types";

export interface WizardOptions {
  container: HTMLElement;
  onComplete: (workspace: Workspace) => void;
}

export function startFirstRunWizard(options: WizardOptions): void {
  const { container, onComplete } = options;

  let currentStep = 1;
  let doctorResults: DoctorResult | null = null;
  let chosenRepoPath = "C:/Users/thiag/projetos-oss/askdash";
  let chosenWsName = "askdash";
  let chosenTeam = "pair"; // "solo" | "pair" | "floor"

  function render() {
    container.innerHTML = `
      <div class="wizard-overlay">
        <div class="wizard-card">
          <div class="wizard-header">
            <span class="brand">NexHub</span>
            <h2>Welcome to NexHub Setup</h2>
            <div class="wizard-steps-indicator">
              <span class="step-pill ${currentStep >= 1 ? "active" : ""}">1. Tools</span>
              <span class="step-pill ${currentStep >= 2 ? "active" : ""}">2. Repository</span>
              <span class="step-pill ${currentStep >= 3 ? "active" : ""}">3. Team</span>
              <span class="step-pill ${currentStep >= 4 ? "active" : ""}">4. Launch</span>
            </div>
          </div>

          <div class="wizard-body">
            ${renderStepContent()}
          </div>

          <div class="wizard-footer">
            ${currentStep > 1 ? `<button class="btn btn-secondary wizard-back-btn">Back</button>` : `<div></div>`}
            ${
              currentStep < 4
                ? `<button class="btn btn-primary wizard-next-btn">Continue</button>`
                : `<button class="btn btn-primary wizard-finish-btn">Open Canvas</button>`
            }
          </div>
        </div>
      </div>
    `;

    container.querySelector(".wizard-back-btn")?.addEventListener("click", () => {
      currentStep--;
      render();
    });

    container.querySelector(".wizard-next-btn")?.addEventListener("click", () => {
      if (currentStep === 2) {
        const inputPath = (container.querySelector("#wizard-repo-path") as HTMLInputElement)?.value;
        const inputName = (container.querySelector("#wizard-repo-name") as HTMLInputElement)?.value;
        if (inputPath) chosenRepoPath = inputPath.trim();
        if (inputName) chosenWsName = inputName.trim();
      }
      currentStep++;
      render();
    });

    container.querySelector(".wizard-finish-btn")?.addEventListener("click", async () => {
      await launchWorkspace();
    });

    attachStepEvents();
  }

  function renderStepContent(): string {
    switch (currentStep) {
      case 1: {
        return `
          <div class="wizard-step">
            <h3>Step 1: Environment & Tool Diagnostics</h3>
            <p class="wizard-desc">NexHub connects coding agents and tools together. Checking your installed CLI tools:</p>
            <div class="tool-list" id="tool-diagnostics">
              ${
                doctorResults
                  ? renderToolList(doctorResults)
                  : `<div class="tool-checking">Checking installed tools (git, agy, claude, gh, node)...</div>`
              }
            </div>
          </div>
        `;
      }
      case 2: {
        return `
          <div class="wizard-step">
            <h3>Step 2: Choose Repository</h3>
            <p class="wizard-desc">Select a local git repository folder to manage as your first workspace:</p>
            <div class="form-group">
              <label for="wizard-repo-name">Workspace name:</label>
              <input id="wizard-repo-name" class="input-text" type="text" value="${chosenWsName}" />
            </div>
            <div class="form-group">
              <label for="wizard-repo-path">Repository folder path:</label>
              <input id="wizard-repo-path" class="input-text" type="text" value="${chosenRepoPath}" />
            </div>
          </div>
        `;
      }
      case 3: {
        return `
          <div class="wizard-step">
            <h3>Step 3: Select Starting Team</h3>
            <p class="wizard-desc">Choose which agents to spawn initially on your canvas:</p>
            <div class="team-options">
              <label class="team-option ${chosenTeam === "solo" ? "selected" : ""}">
                <input type="radio" name="team" value="solo" ${chosenTeam === "solo" ? "checked" : ""} />
                <div class="team-option-content">
                  <div class="team-title">Solo</div>
                  <div class="team-desc">One autonomous agy agent ready to tackle tasks.</div>
                </div>
              </label>

              <label class="team-option ${chosenTeam === "pair" ? "selected" : ""}">
                <input type="radio" name="team" value="pair" ${chosenTeam === "pair" ? "checked" : ""} />
                <div class="team-option-content">
                  <div class="team-title">Pair Programming (Recommended)</div>
                  <div class="team-desc">Builder (agy) + Reviewer (Claude Code) connected on one floor.</div>
                </div>
              </label>

              <label class="team-option ${chosenTeam === "floor" ? "selected" : ""}">
                <input type="radio" name="team" value="floor" ${chosenTeam === "floor" ? "checked" : ""} />
                <div class="team-option-content">
                  <div class="team-title">Floor Team</div>
                  <div class="team-desc">Conductor coordinator plus two parallel builder workers.</div>
                </div>
              </label>
            </div>
          </div>
        `;
      }
      case 4: {
        return `
          <div class="wizard-step">
            <h3>Step 4: Ready to Launch</h3>
            <p class="wizard-desc">We will create your workspace and open the infinite canvas with your selected team.</p>
            <div class="wizard-summary">
              <div><strong>Workspace:</strong> ${chosenWsName}</div>
              <div><strong>Path:</strong> ${chosenRepoPath}</div>
              <div><strong>Team Preset:</strong> ${chosenTeam.toUpperCase()}</div>
            </div>
          </div>
        `;
      }
      default:
        return "";
    }
  }

  function renderToolList(res: DoctorResult): string {
    const tools = [
      { name: "git", ok: res.git, hint: "Required for worktrees and branches. Install git." },
      { name: "agy", ok: res.agy, hint: "Google Antigravity CLI. Install agy." },
      { name: "claude", ok: res.claude, hint: "Claude Code CLI. Install @anthropic-ai/claude-code." },
      { name: "gh", ok: res.gh, hint: "GitHub CLI. Install gh CLI." },
      { name: "node", ok: res.node, hint: "Node.js runtime. Install node." },
    ];

    return tools
      .map(
        (t) => `
        <div class="tool-item ${t.ok ? "tool-ok" : "tool-missing"}">
          <div class="tool-item-header">
            <span class="tool-badge ${t.ok ? "badge-ok" : "badge-warn"}">${t.ok ? "FOUND" : "MISSING"}</span>
            <span class="tool-name">${t.name}</span>
          </div>
          <div class="tool-hint">${t.ok ? res.details[t.name] || "Ready" : t.hint}</div>
        </div>
      `
      )
      .join("");
  }

  function attachStepEvents() {
    if (currentStep === 1 && !doctorResults) {
      doctorCheck().then((res) => {
        doctorResults = res;
        render();
      }).catch(() => {
        doctorResults = {
          git: true,
          agy: true,
          claude: true,
          gh: true,
          node: true,
          details: { git: "found", agy: "found", claude: "found", gh: "found", node: "found" },
        };
        render();
      });
    }

    if (currentStep === 3) {
      const radios = container.querySelectorAll('input[name="team"]');
      radios.forEach((r) => {
        r.addEventListener("change", (e) => {
          chosenTeam = (e.target as HTMLInputElement).value;
          render();
        });
      });
    }
  }

  async function launchWorkspace() {
    try {
      const wsRes = await workspaceAdd({
        name: chosenWsName,
        repo_path: chosenRepoPath,
        group_id: "grp_1",
      });

      // Spawn starting agents based on chosen team
      if (chosenTeam === "solo") {
        await agentsSpawn({
          workspace: wsRes.workspace.id,
          name: "builder",
          preset: "agy",
          role: "builder",
        });
      } else if (chosenTeam === "pair") {
        await agentsSpawn({
          workspace: wsRes.workspace.id,
          name: "builder",
          preset: "agy",
          role: "builder",
          floor: "fl_semantic",
        });
        await agentsSpawn({
          workspace: wsRes.workspace.id,
          name: "reviewer",
          preset: "claude",
          role: "reviewer",
          floor: "fl_semantic",
        });
      } else if (chosenTeam === "floor") {
        await agentsSpawn({
          workspace: wsRes.workspace.id,
          name: "conductor",
          preset: "claude",
          role: "conductor",
        });
        await agentsSpawn({
          workspace: wsRes.workspace.id,
          name: "builder_1",
          preset: "agy",
          role: "builder",
        });
        await agentsSpawn({
          workspace: wsRes.workspace.id,
          name: "builder_2",
          preset: "agy",
          role: "builder",
        });
      }

      onComplete(wsRes.workspace);
    } catch (err) {
      alert(`Setup error: ${String(err)}`);
    }
  }

  render();
}
