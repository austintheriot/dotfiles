# Dotfiles management landscape (2026-09-02)

Research pass done while designing the mac/linux branch drift-check
(see `docs/superpowers/specs/2026-09-02-branch-drift-sync-design.md`).
Question: is there a better model than branch-per-machine for keeping
platform-varying dotfiles in sync? Revisit this if drift keeps recurring
even after the manifest + GitHub Action + tmux.conf split are in place.

## Comparison

| Tool | Model | Solves "one file, few OS lines"? | Secrets | Momentum (2026) | Migration cost from a bare `~/.cfg` repo |
|---|---|---|---|---|---|
| **chezmoi** | Source dir renders to `$HOME` (not symlinks) | **Yes, natively.** Go templates: `dot_config/tmux/tmux.conf.tmpl` with `{{ if eq .chezmoi.os "darwin" }}...{{ else if eq .chezmoi.os "linux" }}...{{ end }}` inline in one file | Built-in: age/GPG/1Password/Bitwarden, safe to make the repo public | 21.4k GitHub stars, shipped a release Aug 30 2026, very active | Moderate — documented migrations run ~2-4 hrs: rename to `dot_*`, port install scripts to `run_once_` |
| **yadm** | Bare-repo wrapper (same core model as this repo) + alt-files (`##os.Darwin`) | Weaker — alt-files mean a whole separate file per OS, not one templated file. Historically leaned on `envtpl`/`j2cli`, both now unmaintained | GPG-encrypted files, built-in | 6.4k stars, maintained but slower-moving | **Lowest** — literally this repo's model plus alt-files/encryption bolted on |
| **GNU Stow / dotbot / rcm** | Symlink farms | **Doesn't solve it** — platform variance means separate directories/whole-file forks, same problem this repo already has | None built-in | Mature but stagnant (Stow); smaller (dotbot) | N/A — not a step up from where this repo is |
| **Nix home-manager (+ nix-darwin, flakes)** | Fully declarative, reproducible config-as-code | Solves it at a deeper level (conditional modules, cross-platform `nixpkgs`), but the config becomes Nix, not `tmux.conf` | Via agenix/sops-nix, more setup | Real 2025-2026 traction among power users, but recent write-ups pair it *with* chezmoi rather than replace it | **Highest** — new mental model, new apply/build loop, every tool needs to be Nix-packaged or shimmed |

## The concrete "few lines differ" case

chezmoi's answer for `tmux.conf`, as an example — the file becomes
`dot_config/tmux/tmux.conf.tmpl`:

```
{{- if eq .chezmoi.os "darwin" }}
bind-key -T copy-mode-vi 'y' send -X copy-pipe "pbcopy"
{{- else if eq .chezmoi.os "linux" }}
bind-key -T copy-mode-vi 'y' send -X copy-pipe "xclip -selection clipboard"
{{- end }}
```

Everything else in the file is written once and never diverges, because
there is only one file. This is a strictly better version of the
common-file-plus-`source-file` split this repo uses instead — no second
file to remember to source, at the cost of adopting chezmoi's templating
and apply/diff workflow.

## Community sentiment

From an HN "Better Dotfiles" thread and related discussion, as of 2026:

- chezmoi is the clear favorite among people who outgrew symlink farms —
  praised for templating, secrets handling, and an actively-engaged
  maintainer.
- Bare git repo (this repo's exact model) still has a real, vocal
  long-term user base — "used this for nearly a decade without issues"
  is a common sentiment. Not considered obsolete, just less capable on
  the templating/secrets axis.
- Consensus is explicitly "no single best tool, depends on your
  tolerance for setup cost," not "bare-repo users are wrong."
- Nix/home-manager users call it "end-game" but often keep chezmoi in
  the loop anyway, specifically to avoid Nix's immutability fighting
  with everyday app configs.

## Recommendation

chezmoi is the legitimate answer if this repo ever migrates to
anything — mature, actively maintained, and it directly beats the
common/platform file split used here. But the actual platform-varying
surface in this repo is small: three files (`tmux.conf`, `.zshrc`,
`alacritty.toml`), with everything else either fully portable already
(`.claude`, `tests`, `.my-scripts`) or fully platform-exclusive
(Aerospace, iTerm, `notify.sh`). The manifest + GitHub Action + tmux.conf
split gets most of chezmoi's practical benefit for the two or three
files that actually need it, with zero migration cost and zero new tool
to learn. Nix is not worth pursuing here at all — its value proposition
(reproducible package management) doesn't match a problem that's
specifically about syncing dotfiles content.

**Decision: stay on the branch-per-machine model. Revisit chezmoi only
if drift keeps recurring even with the manifest + Action + tmux split in
place.**
