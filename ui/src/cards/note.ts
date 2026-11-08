import { notesSet } from "../api";
import { debounce } from "../canvas/layout";
import { icons } from "../icons";

export function sanitizeHtml(html: string): string {
  let clean = html;

  // Strip script tags and their content
  clean = clean.replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, "");

  // Strip iframe, object, embed, style, form, input, button tags
  clean = clean.replace(/<\/?(iframe|object|embed|style|form|input|button)\b[^>]*>/gi, "");

  // Strip event handler attributes (e.g., onerror, onload, onclick, onmouseover, etc.)
  clean = clean.replace(/\s+on[a-z]+\s*=\s*(?:'[^']*'|"[^"]*"|[^\s>]+)/gi, "");

  // Strip javascript:, vbscript:, data:text/html links
  clean = clean.replace(/\bhref\s*=\s*['"]?\s*(?:javascript|vbscript|data\s*:\s*text\/html):/gi, "href=\"#blocked:");
  clean = clean.replace(/\bsrc\s*=\s*['"]?\s*(?:javascript|vbscript|data\s*:\s*text\/html):/gi, "src=\"#blocked:");

  return clean;
}

export function renderMarkdown(markdown: string): string {
  // First strip dangerous HTML like scripts and event handlers
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
  const container = document.createElement("div");
  container.className = "card-inner note-card";
  container.dataset.cardId = cardId;

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
    container.innerHTML = `
      <div class="card-header">
        <div class="card-header-left">
          <span class="card-icon">${icons.note}</span>
          <input class="note-title-input" type="text" value="${currentTitle.replace(/"/g, "&quot;")}" placeholder="Note title" />
        </div>
        <div class="card-header-actions">
          <button class="card-action-btn toggle-view-btn" title="${isEditing ? "Preview" : "Edit"}">
            ${isEditing ? icons.eye : icons.edit}
          </button>
        </div>
      </div>
      <div class="note-body">
        ${
          isEditing
            ? `<textarea class="note-textarea" placeholder="Write markdown...">${currentBody}</textarea>`
            : `<div class="note-preview">${renderMarkdown(currentBody) || '<span class="note-empty">Empty note. Click edit to add content.</span>'}</div>`
        }
      </div>
    `;

    const titleInput = container.querySelector(".note-title-input") as HTMLInputElement;
    titleInput?.addEventListener("change", (e) => {
      currentTitle = (e.target as HTMLInputElement).value.trim() || "untitled";
      if (onTitleChange) {
        onTitleChange(currentTitle);
      }
      debouncedSave(currentTitle, currentBody);
    });

    const toggleBtn = container.querySelector(".toggle-view-btn");
    toggleBtn?.addEventListener("click", () => {
      isEditing = !isEditing;
      render();
    });

    const textarea = container.querySelector(".note-textarea") as HTMLTextAreaElement;
    if (textarea) {
      textarea.addEventListener("input", (e) => {
        currentBody = (e.target as HTMLTextAreaElement).value;
        debouncedSave(currentTitle, currentBody);
      });
      textarea.focus();
    }
  }

  render();

  return {
    element: container,
    saveImmediately: () => {
      debouncedSave.flush();
    },
  };
}
