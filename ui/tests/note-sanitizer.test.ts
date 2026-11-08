import { describe, expect, it } from "vitest";
import { renderMarkdown, sanitizeHtml } from "../src/cards/note";

describe("note markdown renderer and sanitizer", () => {
  it("removes <script> tags completely", () => {
    const raw = `<script>alert('xss')</script>Hello World`;
    const clean = sanitizeHtml(raw);
    expect(clean).not.toContain("<script");
    expect(clean).not.toContain("alert('xss')");
    expect(clean).toContain("Hello World");
  });

  it("removes onerror and other inline event handlers", () => {
    const raw = `<img src="invalid.jpg" onerror="alert(1)" onload="evil()" />`;
    const clean = sanitizeHtml(raw);
    expect(clean).not.toContain("onerror");
    expect(clean).not.toContain("onload");
    expect(clean).not.toContain("alert(1)");
    expect(clean).not.toContain("evil()");
  });

  it("neutralizes javascript: URLs in links and images", () => {
    const raw = `<a href="javascript:alert(1)">Click here</a><img src="javascript:evil()" />`;
    const clean = sanitizeHtml(raw);
    expect(clean).not.toContain("javascript:");
    expect(clean).toBe("<a>Click here</a><img>");
  });

  it("renders safe markdown structures correctly", () => {
    const md = `
# Main Header

## Secondary Header

This is **bold text** and *italic text*.

\`inline code\`

\`\`\`typescript
const greeting = "hello world";
\`\`\`

- [x] Completed task
- [ ] Incomplete task

- Standard bullet item

> Useful blockquote

[Safe link](https://github.com/ThiagoVenturim/nexhub)
    `.trim();

    const html = renderMarkdown(md);

    expect(html).toContain("<h1>Main Header</h1>");
    expect(html).toContain("<h2>Secondary Header</h2>");
    expect(html).toContain("<strong>bold text</strong>");
    expect(html).toContain("<em>italic text</em>");
    expect(html).toContain("<code>inline code</code>");
    expect(html).toContain('<pre><code>const greeting = "hello world";</code></pre>');
    expect(html).toContain('class="task-item checked"');
    expect(html).toContain('class="task-item"');
    expect(html).toContain("<blockquote>Useful blockquote</blockquote>");
    expect(html).toContain('<a href="https://github.com/ThiagoVenturim/nexhub"');
  });

  it("sanitizes malicious markdown inputs with embedded scripts and event handlers", () => {
    const malicious = `
# Notes
<script>stealTokens()</script>
<img src="x" onerror="evilCode()" />
[Click me](javascript:hack())
    `.trim();

    const rendered = renderMarkdown(malicious);

    expect(rendered).not.toContain("<script");
    expect(rendered).not.toContain("stealTokens");
    expect(rendered).not.toContain("onerror");
    expect(rendered).not.toContain("evilCode");
    expect(rendered).not.toContain("javascript:");
    expect(rendered).toContain("<h1>Notes</h1>");
  });
});
