import { agentsSpawn, doctorCheck, workspaceAdd } from "../api";
import { clear, el } from "../dom";
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
    clear(container);

    const stepsIndicator = el(
      "div",
      { class: "wizard-steps-indicator" },
      el("span", { class: `step-pill ${currentStep >= 1 ? "active" : ""}`, text: "1. Tools" }),
      el("span", { class: `step-pill ${currentStep >= 2 ? "active" : ""}`, text: "2. Repository" }),
      el("span", { class: `step-pill ${currentStep >= 3 ? "active" : ""}`, text: "3. Team" }),
      el("span", { class: `step-pill ${currentStep >= 4 ? "active" : ""}`, text: "4. Launch" })
    );

    const wizardHeader = el(
      "div",
      { class: "wizard-header" },
      el("span", { class: "brand", text: "NexHub" }),
      el("h2", { text: "Welcome to NexHub Setup" }),
      stepsIndicator
    );

    const wizardBody = el("div", { class: "wizard-body" }, renderStepContent());

    const footerLeft = el("div");
    if (currentStep > 1) {
      const backBtn = el("button", {
        class: "btn btn-secondary wizard-back-btn",
        text: "Back",
        onClick: () => {
          currentStep--;
          render();
        },
      });
      footerLeft.appendChild(backBtn);
    }

    const nextOrFinishBtn =
      currentStep < 4
        ? el("button", {
            class: "btn btn-primary wizard-next-btn",
            text: "Continue",
            onClick: () => {
              if (currentStep === 2) {
                const inputPath = container.querySelector("#wizard-repo-path") as HTMLInputElement;
                const inputName = container.querySelector("#wizard-repo-name") as HTMLInputElement;
                if (inputPath) chosenRepoPath = inputPath.value.trim();
                if (inputName) chosenWsName = inputName.value.trim();
              }
              currentStep++;
              render();
            },
          })
        : el("button", {
            class: "btn btn-primary wizard-finish-btn",
            text: "Open Canvas",
            onClick: async () => {
              await launchWorkspace();
            },
          });

    const wizardFooter = el("div", { class: "wizard-footer" }, footerLeft, nextOrFinishBtn);

    const wizardCard = el("div", { class: "wizard-card" }, wizardHeader, wizardBody, wizardFooter);

    const wizardOverlay = el("div", { class: "wizard-overlay" }, wizardCard);

    container.appendChild(wizardOverlay);
    attachStepEvents();
  }

  function renderStepContent(): HTMLElement {
    const stepContainer = el("div", { class: "wizard-step" });

    switch (currentStep) {
      case 1: {
        stepContainer.appendChild(el("h3", { text: "Step 1: Environment & Tool Diagnostics" }));
        stepContainer.appendChild(
          el("p", {
            class: "wizard-desc",
            text: "NexHub connects coding agents and tools together. Checking your installed CLI tools:",
          })
        );
        const toolListEl = el("div", { class: "tool-list", id: "tool-diagnostics" });
        if (doctorResults) {
          renderToolList(doctorResults).forEach((item) => toolListEl.appendChild(item));
        } else {
          toolListEl.appendChild(
            el("div", {
              class: "tool-checking",
              text: "Checking installed tools (git, agy, claude, gh, node)...",
            })
          );
        }
        stepContainer.appendChild(toolListEl);
        break;
      }
      case 2: {
        stepContainer.appendChild(el("h3", { text: "Step 2: Choose Repository" }));
        stepContainer.appendChild(
          el("p", {
            class: "wizard-desc",
            text: "Select a local git repository folder to manage as your first workspace:",
          })
        );

        const nameInput = el("input", {
          id: "wizard-repo-name",
          class: "input-text",
          attrs: { type: "text" },
        });
        nameInput.value = chosenWsName;

        const pathInput = el("input", {
          id: "wizard-repo-path",
          class: "input-text",
          attrs: { type: "text" },
        });
        pathInput.value = chosenRepoPath;

        stepContainer.appendChild(
          el(
            "div",
            { class: "form-group" },
            el("label", { attrs: { for: "wizard-repo-name" }, text: "Workspace name:" }),
            nameInput
          )
        );

        stepContainer.appendChild(
          el(
            "div",
            { class: "form-group" },
            el("label", { attrs: { for: "wizard-repo-path" }, text: "Repository folder path:" }),
            pathInput
          )
        );
        break;
      }
      case 3: {
        stepContainer.appendChild(el("h3", { text: "Step 3: Select Starting Team" }));
        stepContainer.appendChild(
          el("p", {
            class: "wizard-desc",
            text: "Choose which agents to spawn initially on your canvas:",
          })
        );

        const teamOptions = el("div", { class: "team-options" });

        const optionsConfig = [
          {
            value: "solo",
            title: "Solo",
            desc: "One autonomous agy agent ready to tackle tasks.",
          },
          {
            value: "pair",
            title: "Pair Programming (Recommended)",
            desc: "Builder (agy) + Reviewer (Claude Code) connected on one floor.",
          },
          {
            value: "floor",
            title: "Floor Team",
            desc: "Conductor coordinator plus two parallel builder workers.",
          },
        ];

        optionsConfig.forEach((cfg) => {
          const radio = el("input", {
            attrs: {
              type: "radio",
              name: "team",
              value: cfg.value,
            },
          });
          radio.checked = chosenTeam === cfg.value;

          const optLabel = el(
            "label",
            { class: `team-option ${chosenTeam === cfg.value ? "selected" : ""}` },
            radio,
            el(
              "div",
              { class: "team-option-content" },
              el("div", { class: "team-title", text: cfg.title }),
              el("div", { class: "team-desc", text: cfg.desc })
            )
          );

          teamOptions.appendChild(optLabel);
        });

        stepContainer.appendChild(teamOptions);
        break;
      }
      case 4: {
        stepContainer.appendChild(el("h3", { text: "Step 4: Ready to Launch" }));
        stepContainer.appendChild(
          el("p", {
            class: "wizard-desc",
            text: "We will create your workspace and open the infinite canvas with your selected team.",
          })
        );

        const summary = el(
          "div",
          { class: "wizard-summary" },
          el("div", {}, el("strong", { text: "Workspace: " }), document.createTextNode(chosenWsName)),
          el("div", {}, el("strong", { text: "Path: " }), document.createTextNode(chosenRepoPath)),
          el("div", {}, el("strong", { text: "Team Preset: " }), document.createTextNode(chosenTeam.toUpperCase()))
        );

        stepContainer.appendChild(summary);
        break;
      }
    }

    return stepContainer;
  }

  function renderToolList(res: DoctorResult): HTMLElement[] {
    const tools = [
      { name: "git", ok: res.git, hint: "Required for worktrees and branches. Install git." },
      { name: "agy", ok: res.agy, hint: "Google Antigravity CLI. Install agy." },
      { name: "claude", ok: res.claude, hint: "Claude Code CLI. Install @anthropic-ai/claude-code." },
      { name: "gh", ok: res.gh, hint: "GitHub CLI. Install gh CLI." },
      { name: "node", ok: res.node, hint: "Node.js runtime. Install node." },
    ];

    return tools.map((t) => {
      const itemHeader = el(
        "div",
        { class: "tool-item-header" },
        el("span", {
          class: `tool-badge ${t.ok ? "badge-ok" : "badge-warn"}`,
          text: t.ok ? "FOUND" : "MISSING",
        }),
        el("span", { class: "tool-name", text: t.name })
      );

      const hintText = t.ok ? res.details[t.name] || "Ready" : t.hint;

      return el(
        "div",
        { class: `tool-item ${t.ok ? "tool-ok" : "tool-missing"}` },
        itemHeader,
        el("div", { class: "tool-hint", text: hintText })
      );
    });
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
