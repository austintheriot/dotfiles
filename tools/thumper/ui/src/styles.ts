import { css } from "lit";

export const tokens = css`
  :host {
    --card-width: 380px;
    --radius: 12px;
    --info-bg: rgba(30, 30, 32, 0.92);
    --info-border: rgba(255, 255, 255, 0.12);
    --attention-bg: rgba(92, 58, 18, 0.95);
    --attention-border: #e0a030;
    --text: #f2f2f2;
    --muted: rgba(242, 242, 242, 0.65);
    --close-bg: rgba(80, 80, 84, 0.95);
    font: 13px/1.35 -apple-system, BlinkMacSystemFont, "Inter", system-ui, sans-serif;
    color: var(--text);
  }
  @media (prefers-color-scheme: light) {
    :host {
      --info-bg: rgba(250, 250, 250, 0.96);
      --info-border: rgba(0, 0, 0, 0.12);
      --attention-bg: rgba(255, 243, 224, 0.97);
      --text: #1d1d1f;
      --muted: rgba(29, 29, 31, 0.6);
      --close-bg: rgba(225, 225, 228, 0.98);
    }
  }
`;
