---
paths:
  - "**/*.{tsx,jsx}"
---

# React

## Reach DOM nodes through refs, not document queries

In a component, get a DOM node from a ref. Do not use `document.getElementById`, `document.querySelector`, `querySelectorAll`, or `getElementsBy*` to find a node that the component renders.

- One node: `useRef` and `ref={...}`.
- A list of nodes (rows, options, cells): a `useRef(new Map<Key, HTMLElement>())` filled by a ref callback per item. In React 19, return a cleanup function from the ref callback to delete the entry.
- A node in a child component: pass a ref prop to the child.

Why: a document query breaks when the component renders outside the main document tree. Examples are a shadow root, a portal into another window, an iframe, or a detached container in a test. A query can also find the wrong node when two instances of the component share an ID or a selector. A ref always points at the node this instance rendered.

Use a document query only as a last resort, when no ref can reach the node: a node that third-party code creates, or a global element such as `document.body` or `document.activeElement`. Add a comment that names the reason.

When you replace a document query with a ref, write the test first. Render the component inside a shadow root: the old query finds nothing there, so the test fails before the fix.

## Deeper references

For client-model questions (effects, state shape, keys, external stores, React Compiler), read `~/.claude/rules/react-components.md` or dispatch the `react-components` agent. For Server Components, Server Functions, framework caching, and hydration, read `~/.claude/rules/react-server.md` or dispatch `react-server`.
