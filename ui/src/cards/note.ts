import DOMPurify from "dompurify";
import { notesSet } from "../api";
import { debounce } from "../canvas/layout";
import { clear, el } from "../dom";
import { icons } from "../icons";

function getPurifier() {
  if (typeof DOMPurify === "function") {
    if (typeof window !== "undefined") {
      return (DOMPurify as unknown as (win: Window) => typeof DOMPurify)(window);
    }
  }
  return DOMPurify;
}

export function sanitizeHtml(html: string): string {
  const purifier = getPurifier();
  return purifier.sanitize(html, {
    USE_PROFILES: { html: true },
  });
}

export function renderMarkdown(markdown: string): string {
  // First strip dangerous script tags and malicious attributes with DOMPurify
  let text = sanitizeHtml(markdown);

  // Code blocks: ```code```
  text = text.replace(/```([a-z0-9_-]*)\n([\s\S]*?)```/gi, (_match, _lang, code) => {
    const escaped = code
      .trim()
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
    return `<pre><code>${escaped}</code></pre>`;
  });

  // Inline code: `code`
  text = text.replace(/`([^`\n]+)`/g, (_match, code) => {
    const escaped = code
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    return `<code>${escaped}</code>`;
  });

  // Headings
  text = text.replace(/^### (.*$)/gim, "<h3>$1</h3>");
  text = text.replace(/^## (.*$)/gim, "<h2>$1</h2>");
  text = text.replace(/^# (.*$)/gim, "<h1>$1</h1>");

  // Blockquotes
  text = text.replace(/^>\s+(.*$)/gim, "<blockquote>$1</blockquote>");

  // Task lists
  text = text.replace(
    /^[-*]\s+\[x\]\s+(.*$)/gim,
    '<div class="task-item checked"><span class="checkbox checked">&#9745;</span> $1</div>'
  );
  text = text.replace(
    /^[-*]\s+\[\s\]\s+(.*$)/gim,
    '<div class="task-item"><span class="checkbox">&#9744;</span> $1</div>'
  );

  // Unordered lists
  text = text.replace(/^[-*]\s+(?!<div)(.*$)/gim, "<li>$1</li>");
  text = text.replace(/(<li>.*<\/li>)/gms, "<ul>$1</ul>");
  text = text.replace(/<\/ul>\s*<ul>/g, "");

  // Bold and italic
  text = text.replace(/\*\*(.*?)\*\*/g, "<strong>$1</strong>");
  text = text.replace(/\*(.*?)\*/g, "<em>$1</em>");

  // Links: [text](url)
  text = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_match, label, url) => {
    const trimmedUrl = url.trim();
    if (/^(https?:|mailto:|#|\/)/i.test(trimmedUrl)) {
      return `<a href="${trimmedUrl}" target="_blank" rel="noopener noreferrer">${label}</a>`;
    }
    return label;
  });

  // Paragraphs / line breaks
  const paragraphs = text
    .split(/\n{2,}/)
    .map((block) => {
      block = block.trim();
      if (!block) {
        return "";
      }
      if (/^<(h[1-3]|ul|div|pre|blockquote)/i.test(block)) {
        return block;
      }
      return `<p>${block.replace(/\n/g, "<br/>")}</p>`;
    })
    .filter(Boolean)
    .join("\n");

  return sanitizeHtml(paragraphs);
}

export interface NoteCardState {
  title: string;
  body: string;
  isEditing: boolean;
  onSave?: (title: string, body: string) => void;
}

export function createNoteCardElement(
  cardId: string,
  initialTitle: string,
  initialBody: string,
  onTitleChange?: (newTitle: string) => void
): {
  element: HTMLElement;
  saveImmediately: () => void;
} {
  const container = el("div", {
    class: "card-inner note-card",
    dataset: { cardId },
  });

  let currentTitle = initialTitle;
  let currentBody = initialBody;
  let isEditing = false;

  const debouncedSave = debounce(async (title: string, body: string) => {
    try {
      await notesSet(title, body);
    } catch (err) {
      console.error("Failed to save note:", err);
    }
  }, 800);

  function render() {
    clear(container);

    const titleInput = el("input", {
      class: "note-title-input",
      attrs: { type: "text", placeholder: "Note title" },
    });
    titleInput.value = currentTitle;

    titleInput.addEventListener("change", (e) => {
      currentTitle = (e.target as HTMLInputElement).value.trim() || "untitled";
      if (onTitleChange) {
        onTitleChange(currentTitle);
      }
      debouncedSave(currentTitle, currentBody);
    });

    const toggleBtn = el(
      "button",
      {
        class: "card-action-btn toggle-view-btn",
        title: isEditing ? "Preview" : "Edit",
        html: isEditing ? icons.eye : icons.edit,
      }
    );

    toggleBtn.addEventListener("click", () => {
      isEditing = !isEditing;
      render();
    });

    const header = el(
      "div",
      { class: "card-header" },
      el("div", { class: "card-header-left" }, el("span", { class: "card-icon", html: icons.note }), titleInput),
      el("div", { class: "card-header-actions" }, toggleBtn)
    );

    const noteBody = el("div", { class: "note-body" });

    if (isEditing) {
      const textarea = el("textarea", {
        class: "note-textarea",
        attrs: { placeholder: "Write markdown..." },
      });
      textarea.value = currentBody;
      textarea.addEventListener("input", (e) => {
        currentBody = (e.target as HTMLTextAreaElement).value;
        debouncedSave(currentTitle, currentBody);
      });
      noteBody.appendChild(textarea);
      setTimeout(() => textarea.focus(), 0);
    } else {
      const preview = el("div", { class: "note-preview" });
      const renderedHtml = renderMarkdown(currentBody);
      if (renderedHtml) {
        preview.innerHTML = renderedHtml;
      } else {
        preview.appendChild(
          el("span", {
            class: "note-empty",
            text: "Empty note. Click edit to add content.",
          })
        );
      }
      noteBody.appendChild(preview);
    }

    container.appendChild(header);
    container.appendChild(noteBody);
  }

  render();

  return {
    element: container,
    saveImmediately: () => {
      debouncedSave.flush();
    },
  };
}
